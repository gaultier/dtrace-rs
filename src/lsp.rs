use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

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
    fn write(writer: &mut impl Write, msg: &str) -> std::io::Result<()> {
        write!(writer, "Content-Length: {}\r\n", msg.len())?;
        writer.write_all(msg.as_bytes())?;
        writer.flush()
    }

    fn read(reader: &mut dyn BufRead) -> std::io::Result<String> {
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

pub fn run(reader: &mut dyn BufRead, writer: &mut impl Write) {
    loop {
        let payload = match Message::read(reader) {
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

        eprintln!("msg={:?}", msg);
    }
}
