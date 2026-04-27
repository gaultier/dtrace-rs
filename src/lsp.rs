use std::{
    collections::HashMap,
    io::{self, BufRead, Write},
};

use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DocumentFormattingParams, Hover, HoverContents, HoverParams,
    HoverProviderCapability, Location, MarkedString, OneOf, PositionEncodingKind,
    PublishDiagnosticsParams, Range, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions, TextEdit, Uri,
};
use serde::{Deserialize, Serialize};

use crate::{CompileResult, compile, fmt, origin::Origin};

enum State {
    Initial,
    Initialized {
        docs: HashMap<Uri, (String, CompileResult)>,
    },
    ShuttingDown,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Request {
    pub id: RequestId,
    pub method: String,
    #[serde(default = "serde_json::Value::default")]
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Response {
    // JSON-RPC allows this to be null if we can't find or parse the
    // request id. We fail deserialization in that case, so we just
    // make this field mandatory.
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<ResponseError>,
}

#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum ErrorCode {
    // Defined by JSON RPC:
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
    ServerErrorStart = -32099,
    ServerErrorEnd = -32000,

    /// Error code indicating that a server received a notification or
    /// request before the server has received the `initialize` request.
    ServerNotInitialized = -32002,
    UnknownErrorCode = -32001,

    // Defined by the protocol:
    /// The client has canceled a request and a server has detected
    /// the cancel.
    RequestCanceled = -32800,

    /// The server detected that the content of a document got
    /// modified outside normal conditions. A server should
    /// NOT send this error code if it detects a content change
    /// in it unprocessed messages. The result even computed
    /// on an older state might still be useful for the client.
    ///
    /// If a client decides that a result is not of any use anymore
    /// the client should cancel the request.
    ContentModified = -32801,

    /// The server cancelled the request. This error code should
    /// only be used for requests that explicitly support being
    /// server cancellable.
    ///
    /// @since 3.17.0
    ServerCancelled = -32802,

    /// A request failed but it was syntactically correct, e.g the
    /// method name was known and the parameters were valid. The error
    /// message should contain human readable information about why
    /// the request failed.
    ///
    /// @since 3.17.0
    RequestFailed = -32803,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct RequestId(IdRepr);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(untagged)]
enum IdRepr {
    I32(i32),
    String(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
    pub method: String,
    #[serde(default = "serde_json::Value::default")]
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub params: serde_json::Value,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Message {
    Request(Request),
    Response(Response),
    Notification(Notification),
}

fn origin_to_lsp_range(origin: Origin) -> lsp_types::Range {
    lsp_types::Range {
        start: lsp_types::Position {
            line: origin.start.line - 1,
            character: origin.start.column - 1,
        },
        end: lsp_types::Position {
            line: origin.end.line - 1,
            character: origin.end.column - 1,
        },
    }
}

impl Message {
    fn write_payload(writer: &mut impl Write, msg: &str) -> std::io::Result<()> {
        write!(writer, "Content-Length: {}\r\n\r\n", msg.len())?;
        writer.write_all(msg.as_bytes())?;
        writer.flush()
    }

    fn write(&self, writer: &mut impl Write) -> std::io::Result<()> {
        #[derive(Serialize)]
        struct JsonRpc<'a> {
            jsonrpc: &'static str,
            #[serde(flatten)]
            msg: &'a Message,
        }
        let json_rpc = JsonRpc {
            jsonrpc: "2.0",
            msg: self,
        };

        let j = serde_json::to_string(&json_rpc)?;
        Message::write_payload(writer, &j)
    }

    fn read_payload(reader: &mut dyn BufRead) -> std::io::Result<String> {
        let mut buf = String::with_capacity(8192);
        let mut size: Option<usize> = None;

        for _ in 0..100 {
            buf.clear();

            if reader.read_line(&mut buf)? == 0 {
                return Ok(String::new());
            }

            if !buf.ends_with("\r\n") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "missing CRLF after header",
                ));
            }
            let buf = &buf[..buf.len() - 2];

            if buf.is_empty() {
                // Start of real data.
                break;
            }

            let mut parts = buf.splitn(3, ": ");
            let header_name = parts.next().ok_or(io::Error::new(
                io::ErrorKind::InvalidData,
                "malformed header",
            ))?;
            let header_value = parts.next().ok_or(io::Error::new(
                io::ErrorKind::InvalidData,
                "malformed header",
            ))?;
            if parts.next().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "malformed header",
                ));
            }

            if header_name.eq_ignore_ascii_case("Content-Length") {
                size = Some(header_value.parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid content length: {}", header_value),
                    )
                })?);
            }
        }

        let size = size.ok_or(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing content length",
        ))?;

        let mut buf = buf.into_bytes();
        buf.resize(size, 0);
        reader.read_exact(&mut buf)?;
        let buf = String::from_utf8(buf)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid utf8"))?;
        Ok(buf)
    }
}

fn make_server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        position_encoding: Some(PositionEncodingKind::UTF8),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                ..Default::default()
            },
        )),
        document_formatting_provider: Some(OneOf::Left(true)),
        ..Default::default()
    }
}

fn hover(state: &State, id: RequestId, params: serde_json::Value) -> io::Result<Option<Message>> {
    let docs = match state {
        State::Initialized { docs, .. } => Ok(docs),
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "invalid state")),
    }?;
    let params: HoverParams = serde_json::from_value(params).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid params: {}", err),
        )
    })?;
    let (text, compiled) = docs
        .get(&params.text_document_position_params.text_document.uri)
        .ok_or(io::Error::new(
            io::ErrorKind::InvalidData,
            "unknown document",
        ))?;

    let pos = params.text_document_position_params.position;
    // LSP positions are (line, character) where `character` is a UTF-8 byte offset
    // within the line (since we negotiate UTF-8 encoding). Convert to a file-level
    // byte offset so it can be compared against `Origin::byte_offset`.
    let line_start_byte: u32 = text
        .split('\n')
        .take(pos.line as usize)
        .map(|l| l.len() as u32 + 1) // +1 for the '\n'
        .sum();
    let cursor_byte = line_start_byte + pos.character;
    // FIXME: No need to allocate all the strings before we have picked the most specific (i.e.
    // inner-most) element.
    let found = compiled
        .ast_nodes
        .iter()
        .filter(|x| {
            x.origin.start.byte_offset <= cursor_byte && cursor_byte < x.origin.end.byte_offset
        })
        .map(|x| (x.origin, format!("ast node: {:?}", x.kind)))
        .chain(
            compiled
                .control_directives
                .iter()
                .filter(|x| {
                    x.origin.start.byte_offset <= cursor_byte
                        && cursor_byte < x.origin.end.byte_offset
                })
                .map(|x| (x.origin, format!("control directive: {:?}", x.kind))),
        )
        .chain(
            compiled
                .attributes
                .iter()
                .filter(|x| {
                    x.origin.start.byte_offset <= cursor_byte
                        && cursor_byte < x.origin.end.byte_offset
                })
                .map(|x| (x.origin, String::from("attribute"))),
        )
        .chain(
            compiled
                .comments
                .iter()
                .filter(|x| {
                    x.origin.start.byte_offset <= cursor_byte
                        && cursor_byte < x.origin.end.byte_offset
                })
                .map(|x| (x.origin, format!("comment: {:?}", x.kind))),
        )
        .min_by(|(a, _): &(Origin, _), (b, _): &(Origin, _)| a.len().cmp(&b.len()));
    let resp = if let Some((origin, marked_string)) = found {
        let hover = Hover {
            contents: HoverContents::Scalar(MarkedString::String(marked_string)),
            range: Some(origin_to_lsp_range(origin)),
        };
        serde_json::to_value(&hover).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid params: {}", err),
            )
        })?
    } else {
        serde_json::Value::Null
    };

    Ok(Some(Message::Response(Response {
        id,
        result: Some(resp),
        error: None,
    })))
}

// Converts a slice of compiler errors into LSP diagnostics.
//
// For each error that carries a `related_origin` (e.g., a redeclaration), two diagnostics
// are emitted:
//   1. An ERROR at the offending site with `relatedInformation` pointing to the original.
//   2. A HINT at the original site with `relatedInformation` pointing back to the offending
//      site, so that editors that do not underline `relatedInformation` locations still mark
//      the original declaration.
fn errors_to_diagnostics(errors: &[crate::error::Error], uri: &Uri) -> Vec<Diagnostic> {
    errors
        .iter()
        .flat_map(|e| {
            let message = if e.explanation.is_empty() {
                format!("{:?}", e.kind)
            } else {
                e.explanation.clone()
            };

            let main = Diagnostic {
                range: origin_to_lsp_range(e.origin),
                severity: Some(DiagnosticSeverity::ERROR),
                message: message.clone(),
                // FIXME: This assumes that the related origin is in the same file.
                related_information: e.related_origin.map(|rel| {
                    vec![DiagnosticRelatedInformation {
                        location: Location {
                            uri: uri.clone(),
                            range: origin_to_lsp_range(rel),
                        },
                        message: String::from("First declared here"),
                    }]
                }),
                ..Default::default()
            };

            // Emit a secondary hint at the original declaration so that editors which do not
            // underline `relatedInformation` locations still highlight it.
            let hint = e.related_origin.map(|rel| Diagnostic {
                range: origin_to_lsp_range(rel),
                severity: Some(DiagnosticSeverity::HINT),
                message,
                related_information: Some(vec![DiagnosticRelatedInformation {
                    location: Location {
                        uri: uri.clone(),
                        range: origin_to_lsp_range(e.origin),
                    },
                    message: String::from("Redeclared here"),
                }]),
                ..Default::default()
            });

            std::iter::once(main).chain(hint)
        })
        .collect()
}

fn did_open(state: &mut State, params: serde_json::Value) -> io::Result<Option<Message>> {
    let docs = match state {
        State::Initialized { docs } => docs,
        _ => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid state"));
        }
    };
    let params: DidOpenTextDocumentParams = serde_json::from_value(params).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid params: {}", err),
        )
    })?;

    // FIXME
    let compiled = compile(&params.text_document.text, 1);
    let resp = PublishDiagnosticsParams {
        uri: params.text_document.uri.clone(),
        diagnostics: errors_to_diagnostics(&compiled.errors, &params.text_document.uri),
        version: Some(params.text_document.version),
    };
    docs.insert(
        params.text_document.uri.clone(),
        (params.text_document.text, compiled),
    );
    Ok(Some(Message::Notification(Notification {
        method: String::from("textDocument/publishDiagnostics"),
        params: serde_json::to_value(resp).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to encode: {}", err),
            )
        })?,
    })))
}

fn did_change(state: &mut State, params: serde_json::Value) -> Result<Option<Message>, io::Error> {
    let docs = match state {
        State::Initialized { docs } => docs,
        _ => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid state"));
        }
    };
    let params: DidChangeTextDocumentParams = serde_json::from_value(params).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid params: {}", err),
        )
    })?;

    let text = &params.content_changes[0].text;
    let compiled = compile(text, 1);
    let resp = PublishDiagnosticsParams {
        uri: params.text_document.uri.clone(),
        diagnostics: errors_to_diagnostics(&compiled.errors, &params.text_document.uri),
        version: Some(params.text_document.version),
    };
    docs.insert(params.text_document.uri.clone(), (text.clone(), compiled));
    Ok(Some(Message::Notification(Notification {
        method: String::from("textDocument/publishDiagnostics"),
        params: serde_json::to_value(resp).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to encode: {}", err),
            )
        })?,
    })))
}

fn handle(msg: Message, state: &mut State) -> io::Result<Option<Message>> {
    match msg {
        Message::Request(Request { method: m, id, .. }) if m == "initialize" => {
            let server_capabilities = make_server_capabilities();
            let initialize_data = serde_json::json!({
                "capabilities": server_capabilities,
                "serverInfo": {
                    "name": "dtrace",
                    "version": "0.1"
                }
            });

            let resp = Message::Response(Response {
                id,
                result: Some(initialize_data),
                error: None,
            });

            *state = State::Initialized {
                docs: HashMap::new(),
            };

            Ok(Some(resp))
        }
        Message::Request(Request { method: m, id, .. }) if m == "shutdown" => {
            *state = State::ShuttingDown;
            Ok(Some(Message::Response(Response {
                id,
                result: Some(serde_json::Value::Null),
                error: None,
            })))
        }
        Message::Request(Request {
            method: m,
            id,
            params,
        }) if m == "textDocument/hover" => hover(state, id, params),
        Message::Request(Request {
            method: m,
            id,
            params,
        }) if m == "textDocument/formatting" => formatting(state, id, params),
        Message::Notification(Notification { method: m, params })
            if m == "textDocument/didOpen" =>
        {
            did_open(state, params)
        }
        Message::Notification(Notification { method: m, params })
            if m == "textDocument/didChange" =>
        {
            did_change(state, params)
        }
        Message::Notification(Notification { method: m, .. }) if m == "exit" => match state {
            State::ShuttingDown => std::process::exit(0),
            _ => std::process::exit(1),
        },
        Message::Request(Request { method: m, .. }) if m == "initialized" => Ok(None),
        Message::Request(_) => Ok(None),
        Message::Response(_response) => Ok(None),
        Message::Notification(_notification) => Ok(None),
    }
}

fn formatting(
    state: &mut State,
    id: RequestId,
    params: serde_json::Value,
) -> Result<Option<Message>, io::Error> {
    let docs = match state {
        State::Initialized { docs, .. } => Ok(docs),
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "invalid state")),
    }?;
    let params: DocumentFormattingParams = serde_json::from_value(params).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid params: {}", err),
        )
    })?;
    let (text, compiled) = docs.get(&params.text_document.uri).ok_or(io::Error::new(
        io::ErrorKind::InvalidData,
        "unknown document",
    ))?;

    let root = if let Some(root) = compiled.ast_root {
        root
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ast root not found",
        ));
    };

    let mut formatted_bytes = Vec::with_capacity(1024);
    fmt::format(
        &mut formatted_bytes,
        root,
        &compiled.ast_nodes,
        &compiled.comments,
        &compiled.control_directives,
        &compiled.attributes,
        text,
    )
    .map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to format: {}", err),
        )
    })?;
    let formatted = String::from_utf8(formatted_bytes).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to format: {}", err),
        )
    })?;

    let text_edit = if formatted == *text {
        vec![]
    } else {
        vec![TextEdit {
            range: Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: text.bytes().filter(|&b| b == b'\n').count() as u32,
                    character: text
                        .rfind('\n')
                        .map_or(text.len(), |pos| text.len() - pos - 1)
                        as u32,
                },
            },
            new_text: formatted,
        }]
    };
    let resp = serde_json::to_value(&text_edit).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid params: {}", err),
        )
    })?;

    Ok(Some(Message::Response(Response {
        id,
        result: Some(resp),
        error: None,
    })))
}

pub fn run(reader: &mut dyn BufRead, writer: &mut impl Write) {
    let mut state = State::Initial;
    loop {
        let payload = match Message::read_payload(reader) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("failed to read message: {:?}", err);
                continue;
            }
        };
        let msg: Message = match serde_json::from_str(&payload) {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("malformed LSP payload: {} {}", e, payload);
                continue;
            }
        };

        match handle(msg, &mut state) {
            Ok(Some(resp)) => {
                resp.write(writer).unwrap();
            }
            Ok(None) => {}
            Err(err) => {
                eprintln!("handle error={}", err);
            }
        }
    }
}
