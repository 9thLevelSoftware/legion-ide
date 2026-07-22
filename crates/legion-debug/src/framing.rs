//! DAP stdio framing (`Content-Length` headers) with the **Microsoft Debug
//! Adapter Protocol** message shape (`seq` / `type` / `command`|`event` /
//! `arguments`|`body` / `request_seq` / `success`).
//!
//! Same framing family as LSP (`ADR-0034` / `legion-lsp`); kept local so
//! `legion-debug` does not depend on the LSP crate.
//!
//! Spec: <https://microsoft.github.io/debug-adapter-protocol/specification>

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Maximum accepted frame payload (1 MiB). DAP responses are small metadata.
pub const MAX_FRAME_PAYLOAD_BYTES: usize = 1024 * 1024;

/// Framing / JSON errors for DAP wire messages.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DapFrameError {
    /// Frame headers or body were malformed.
    #[error("malformed DAP frame: {message}")]
    Malformed {
        /// Bounded diagnostic.
        message: String,
    },
    /// JSON serialization or parse failure.
    #[error("DAP JSON error: {message}")]
    Json {
        /// Bounded diagnostic.
        message: String,
    },
}

/// Microsoft DAP protocol message (`ProtocolMessage` + request/response/event).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DapMessage {
    /// Client → adapter request.
    #[serde(rename = "request")]
    Request {
        /// Sequence number (unique per sender).
        seq: u64,
        /// DAP command name (`initialize`, `launch`, …).
        command: String,
        /// Command arguments.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arguments: Option<Value>,
    },
    /// Adapter → client response to a request.
    #[serde(rename = "response")]
    Response {
        /// Sequence number.
        seq: u64,
        /// Sequence of the corresponding request.
        request_seq: u64,
        /// Whether the request succeeded.
        success: bool,
        /// Command that this is a response to.
        command: String,
        /// Optional human-readable error or status.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        /// Response body.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body: Option<Value>,
    },
    /// Adapter → client event.
    #[serde(rename = "event")]
    Event {
        /// Sequence number.
        seq: u64,
        /// Event name (`initialized`, `stopped`, …).
        event: String,
        /// Event body.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body: Option<Value>,
    },
}

impl DapMessage {
    /// Build a request with a numeric sequence id.
    pub fn request(seq: u64, command: impl Into<String>, arguments: Value) -> Self {
        Self::Request {
            seq,
            command: command.into(),
            arguments: Some(arguments),
        }
    }

    /// Build a successful response.
    pub fn success_response(
        seq: u64,
        request_seq: u64,
        command: impl Into<String>,
        body: Value,
    ) -> Self {
        Self::Response {
            seq,
            request_seq,
            success: true,
            command: command.into(),
            message: None,
            body: Some(body),
        }
    }

    /// Build a failure response.
    pub fn error_response(
        seq: u64,
        request_seq: u64,
        command: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Response {
            seq,
            request_seq,
            success: false,
            command: command.into(),
            message: Some(message.into()),
            body: None,
        }
    }

    /// Build an event.
    pub fn event(seq: u64, event: impl Into<String>, body: Value) -> Self {
        Self::Event {
            seq,
            event: event.into(),
            body: Some(body),
        }
    }

    /// Request sequence when this is a request, else `None`.
    pub fn request_seq_id(&self) -> Option<u64> {
        match self {
            Self::Request { seq, .. } => Some(*seq),
            _ => None,
        }
    }

    /// Event name when this is an event.
    pub fn event_name(&self) -> Option<&str> {
        match self {
            Self::Event { event, .. } => Some(event.as_str()),
            _ => None,
        }
    }

    /// Event body when this is an event.
    pub fn event_body(&self) -> Option<&Value> {
        match self {
            Self::Event { body, .. } => body.as_ref(),
            _ => None,
        }
    }

    /// Response body when this is a successful response for `request_seq`.
    pub fn response_for(&self, request_seq: u64) -> Option<Result<&Value, String>> {
        match self {
            Self::Response {
                request_seq: rs,
                success,
                body,
                message,
                ..
            } if *rs == request_seq => {
                if *success {
                    Some(Ok(body.as_ref().unwrap_or(&Value::Null)))
                } else {
                    Some(Err(message
                        .clone()
                        .unwrap_or_else(|| "request failed".to_string())))
                }
            }
            _ => None,
        }
    }
}

/// Encode/decode DAP `Content-Length` frames carrying [`DapMessage`].
pub struct DapFramer;

impl DapFramer {
    /// Encode one message to a framed byte buffer.
    pub fn encode(message: &DapMessage) -> Result<Vec<u8>, DapFrameError> {
        let payload = serde_json::to_vec(message).map_err(|err| DapFrameError::Json {
            message: err.to_string(),
        })?;
        if payload.len() > MAX_FRAME_PAYLOAD_BYTES {
            return Err(DapFrameError::Malformed {
                message: format!(
                    "payload {} exceeds max {MAX_FRAME_PAYLOAD_BYTES}",
                    payload.len()
                ),
            });
        }
        let mut frame = format!("Content-Length: {}\r\n\r\n", payload.len()).into_bytes();
        frame.extend_from_slice(&payload);
        Ok(frame)
    }

    /// Decode one complete frame (headers + body) into a message.
    pub fn decode(frame: &[u8]) -> Result<DapMessage, DapFrameError> {
        let header_end = frame
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or_else(|| DapFrameError::Malformed {
                message: "missing header separator".to_string(),
            })?;
        let header =
            std::str::from_utf8(&frame[..header_end]).map_err(|err| DapFrameError::Malformed {
                message: format!("header was not UTF-8: {err}"),
            })?;
        let length = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("Content-Length").then_some(value)
            })
            .ok_or_else(|| DapFrameError::Malformed {
                message: "missing Content-Length header".to_string(),
            })?
            .trim()
            .parse::<usize>()
            .map_err(|err| DapFrameError::Malformed {
                message: format!("invalid Content-Length: {err}"),
            })?;
        if length > MAX_FRAME_PAYLOAD_BYTES {
            return Err(DapFrameError::Malformed {
                message: format!("Content-Length {length} exceeds max {MAX_FRAME_PAYLOAD_BYTES}"),
            });
        }
        let payload_start = header_end + 4;
        let payload_end = payload_start.saturating_add(length);
        if frame.len() < payload_end {
            return Err(DapFrameError::Malformed {
                message: "frame shorter than Content-Length".to_string(),
            });
        }
        serde_json::from_slice(&frame[payload_start..payload_end]).map_err(|err| {
            DapFrameError::Json {
                message: err.to_string(),
            }
        })
    }

    /// Read one frame from a buffered reader (blocking).
    pub fn read_from<R: std::io::BufRead>(reader: &mut R) -> Result<DapMessage, DapFrameError> {
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            let n = reader
                .read_line(&mut line)
                .map_err(|err| DapFrameError::Malformed {
                    message: format!("header read failed: {err}"),
                })?;
            if n == 0 {
                return Err(DapFrameError::Malformed {
                    message: "unexpected EOF in headers".to_string(),
                });
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
            headers.push(line);
        }
        let header_text = headers.join("");
        let length = header_text
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("Content-Length").then_some(value)
            })
            .ok_or_else(|| DapFrameError::Malformed {
                message: "missing Content-Length header".to_string(),
            })?
            .trim()
            .parse::<usize>()
            .map_err(|err| DapFrameError::Malformed {
                message: format!("invalid Content-Length: {err}"),
            })?;
        if length > MAX_FRAME_PAYLOAD_BYTES {
            return Err(DapFrameError::Malformed {
                message: format!("Content-Length {length} exceeds max {MAX_FRAME_PAYLOAD_BYTES}"),
            });
        }
        let mut payload = vec![0u8; length];
        reader
            .read_exact(&mut payload)
            .map_err(|err| DapFrameError::Malformed {
                message: format!("payload read failed: {err}"),
            })?;
        serde_json::from_slice(&payload).map_err(|err| DapFrameError::Json {
            message: err.to_string(),
        })
    }

    /// Write one framed message.
    pub fn write_to<W: std::io::Write>(
        writer: &mut W,
        message: &DapMessage,
    ) -> Result<(), DapFrameError> {
        let frame = Self::encode(message)?;
        writer
            .write_all(&frame)
            .map_err(|err| DapFrameError::Malformed {
                message: format!("write failed: {err}"),
            })?;
        writer.flush().map_err(|err| DapFrameError::Malformed {
            message: format!("flush failed: {err}"),
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn encode_decode_request_roundtrip() {
        let msg = DapMessage::request(1, "initialize", json!({"adapterID": "legion-fake"}));
        let frame = DapFramer::encode(&msg).expect("encode");
        assert!(frame.starts_with(b"Content-Length:"));
        let decoded = DapFramer::decode(&frame).expect("decode");
        match decoded {
            DapMessage::Request {
                seq,
                command,
                arguments,
            } => {
                assert_eq!(seq, 1);
                assert_eq!(command, "initialize");
                assert_eq!(
                    arguments
                        .as_ref()
                        .and_then(|a| a.get("adapterID"))
                        .and_then(|v| v.as_str()),
                    Some("legion-fake")
                );
            }
            other => panic!("expected request, got {other:?}"),
        }
    }

    #[test]
    fn microsoft_dap_shape_has_type_field() {
        let msg = DapMessage::request(2, "launch", json!({"program": "a.out"}));
        let json = serde_json::to_value(&msg).expect("to_value");
        assert_eq!(json.get("type").and_then(|v| v.as_str()), Some("request"));
        assert_eq!(json.get("command").and_then(|v| v.as_str()), Some("launch"));
        assert!(json.get("jsonrpc").is_none());
        assert!(json.get("method").is_none());
    }

    #[test]
    fn response_and_event_shapes() {
        let resp = DapMessage::success_response(
            10,
            2,
            "initialize",
            json!({"supportsConfigurationDoneRequest": true}),
        );
        let ev = DapMessage::event(11, "initialized", json!({}));
        let resp_json = serde_json::to_value(&resp).unwrap();
        let ev_json = serde_json::to_value(&ev).unwrap();
        assert_eq!(resp_json["type"], "response");
        assert_eq!(resp_json["request_seq"], 2);
        assert_eq!(resp_json["success"], true);
        assert_eq!(ev_json["type"], "event");
        assert_eq!(ev_json["event"], "initialized");
    }
}
