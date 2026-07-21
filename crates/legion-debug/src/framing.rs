//! DAP stdio JSON-RPC framing (`Content-Length` headers).
//!
//! Same framing family as LSP (`ADR-0034` / `legion-lsp`); kept local so
//! `legion-debug` does not depend on the LSP crate.

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

/// JSON-RPC 2.0 envelope used on the DAP wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DapJsonRpc {
    /// Protocol version (always `"2.0"`).
    pub jsonrpc: String,
    /// Request/response id (integer or string in DAP; we use u64 client ids).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    /// Method for requests and events/notifications.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Params for requests/events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    /// Result for successful responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error for failed responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

impl DapJsonRpc {
    /// Build a request with a numeric id.
    pub fn request(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(id)),
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Build a notification / event (no id).
    pub fn notification(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    /// Build a success response.
    pub fn response(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: None,
            params: None,
            result: Some(result),
            error: None,
        }
    }
}

/// Encode/decode DAP `Content-Length` frames.
pub struct DapFramer;

impl DapFramer {
    /// Encode one envelope to a framed byte buffer.
    pub fn encode(message: &DapJsonRpc) -> Result<Vec<u8>, DapFrameError> {
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

    /// Decode one complete frame (headers + body) into an envelope.
    pub fn decode(frame: &[u8]) -> Result<DapJsonRpc, DapFrameError> {
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
    pub fn read_from<R: std::io::BufRead>(reader: &mut R) -> Result<DapJsonRpc, DapFrameError> {
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
        message: &DapJsonRpc,
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
    fn encode_decode_roundtrip() {
        let msg = DapJsonRpc::request(1, "initialize", json!({"adapterID": "legion-fake"}));
        let frame = DapFramer::encode(&msg).expect("encode");
        assert!(frame.starts_with(b"Content-Length:"));
        let decoded = DapFramer::decode(&frame).expect("decode");
        assert_eq!(decoded.method.as_deref(), Some("initialize"));
        assert_eq!(decoded.id, Some(Value::from(1u64)));
    }
}
