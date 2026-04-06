use std::{
    collections::HashMap,
    io::{self, BufRead, Write},
};

use lsp_types::{
    DidOpenTextDocumentParams, HoverProviderCapability, PositionEncodingKind, ServerCapabilities,
    TextDocumentItem, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    Uri,
};
use serde::{Deserialize, Serialize};

use crate::{CompileResult, compile};

enum State {
    Initial,
    Initialized {
        docs: HashMap<Uri, (TextDocumentItem, CompileResult)>,
        file_id_to_name: HashMap<u32, String>,
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
        ..Default::default()
    }
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
                file_id_to_name: HashMap::new(),
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
        Message::Request(Request { method: m, id, .. }) if m == "textDocument/hover" => {
            // TODO
            Ok(Some(Message::Response(Response {
                id,
                result: Some(serde_json::Value::Null),
                error: None,
            })))
        }
        Message::Notification(Notification { method: m, params })
            if m == "textDocument/didOpen" =>
        {
            let (docs, file_id_to_name) = match state {
                State::Initialized {
                    docs,
                    file_id_to_name,
                } => (docs, file_id_to_name),
                _ => unreachable!(),
            };
            let params: DidOpenTextDocumentParams = serde_json::from_value(params).unwrap();
            let s = params.text_document.uri.as_str().to_owned();
            file_id_to_name.insert(1, s);
            let compiled = compile(&params.text_document.text, 1, &file_id_to_name);
            eprintln!(
                "compiled: {}",
                serde_json::to_string(&compiled).unwrap_or_default()
            );
            docs.insert(
                params.text_document.uri.clone(),
                (params.text_document, compiled),
            );

            Ok(None)
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
