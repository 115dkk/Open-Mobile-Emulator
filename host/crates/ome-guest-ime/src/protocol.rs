// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use std::io::{self, BufRead, Read};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Protocol version implemented by the host.
pub const PROTOCOL_VERSION: u32 = 1;
/// Maximum frame body before the terminating newline.
pub const MAX_LINE_BYTES: usize = 64 * 1024;

/// Non-text keys carried by the text-input link while an editor owns focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextKey {
    Enter,
    Backspace,
    Delete,
    Tab,
    Escape,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

/// Frames sent from the host to the guest input method.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum HostMessage {
    Hello { version: u32 },
    Compose { text: String },
    Commit { text: String },
    Key { key: TextKey },
}

/// Editor metadata supplied by Android on focus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusInfo {
    pub input_type: u32,
    pub ime_action: u32,
    pub package: String,
}

/// Frames sent from the guest input method to the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuestMessage {
    Hello {
        version: u32,
        package: String,
        version_code: u64,
    },
    Focus(FocusInfo),
    Blur,
    /// A well-formed JSON object whose `op` the current host does not know.
    Unknown {
        op: String,
    },
}

#[derive(Debug, Deserialize)]
struct GuestFrame {
    op: String,
    #[serde(flatten)]
    fields: serde_json::Map<String, Value>,
}

/// Errors while framing or decoding the bounded protocol.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("IME frame exceeds 64 KiB")]
    LineTooLong,
    #[error("IME frame is invalid JSON")]
    Json(#[from] serde_json::Error),
    #[error("IME frame could not be read")]
    Io(#[from] io::Error),
}

/// Encodes one host frame, including its newline terminator.
pub fn encode(message: &HostMessage) -> Result<Vec<u8>, ProtocolError> {
    let mut frame = serde_json::to_vec(message)?;
    if frame.len() > MAX_LINE_BYTES {
        return Err(ProtocolError::LineTooLong);
    }
    frame.push(b'\n');
    Ok(frame)
}

/// Decodes one guest frame without requiring a trailing newline.
pub fn decode(line: &[u8]) -> Result<GuestMessage, ProtocolError> {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    if line.len() > MAX_LINE_BYTES {
        return Err(ProtocolError::LineTooLong);
    }
    let frame: GuestFrame = serde_json::from_slice(line)?;
    let value = Value::Object(frame.fields);
    match frame.op.as_str() {
        "hello" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Hello {
                version: u32,
                package: String,
                version_code: u64,
            }
            let hello: Hello = serde_json::from_value(value)?;
            Ok(GuestMessage::Hello {
                version: hello.version,
                package: hello.package,
                version_code: hello.version_code,
            })
        }
        "focus" => Ok(GuestMessage::Focus(serde_json::from_value(value)?)),
        "blur" => Ok(GuestMessage::Blur),
        op => Ok(GuestMessage::Unknown { op: op.to_owned() }),
    }
}

/// Reads one bounded line. EOF before any bytes returns `Ok(None)`.
pub fn read_line<R: BufRead>(reader: &mut R) -> Result<Option<Vec<u8>>, ProtocolError> {
    let mut line = Vec::new();
    let limit = u64::try_from(MAX_LINE_BYTES).expect("constant fits u64") + 2;
    let read = Read::take(&mut *reader, limit).read_until(b'\n', &mut line)?;
    if read == 0 {
        return Ok(None);
    }
    let without_newline = line.strip_suffix(b"\n").unwrap_or(&line);
    let body_len = without_newline
        .strip_suffix(b"\r")
        .unwrap_or(without_newline)
        .len();
    if body_len > MAX_LINE_BYTES || !line.ends_with(b"\n") {
        return Err(ProtocolError::LineTooLong);
    }
    Ok(Some(line))
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use super::*;

    #[test]
    fn host_frames_round_trip_and_use_lowercase_keys() {
        let messages = [
            HostMessage::Hello { version: 1 },
            HostMessage::Compose {
                text: "한".to_owned(),
            },
            HostMessage::Commit {
                text: "한글".to_owned(),
            },
            HostMessage::Key { key: TextKey::End },
        ];
        for message in messages {
            let encoded = encode(&message).expect("encode");
            assert!(encoded.ends_with(b"\n"));
            let decoded: HostMessage = serde_json::from_slice(&encoded).expect("decode host");
            assert_eq!(decoded, message);
        }
        assert_eq!(
            String::from_utf8(
                encode(&HostMessage::Key {
                    key: TextKey::Backspace
                })
                .expect("key")
            )
            .expect("UTF-8"),
            "{\"op\":\"key\",\"key\":\"backspace\"}\n"
        );
    }

    #[test]
    fn guest_frames_decode() {
        assert_eq!(
            decode(br#"{"op":"hello","version":1,"package":"org.openmobileemulator.ime","versionCode":7}"#)
                .expect("hello"),
            GuestMessage::Hello {
                version: 1,
                package: "org.openmobileemulator.ime".to_owned(),
                version_code: 7,
            }
        );
        assert_eq!(
            decode(br#"{"op":"focus","inputType":1,"imeAction":6,"package":"com.example.app"}"#)
                .expect("focus"),
            GuestMessage::Focus(FocusInfo {
                input_type: 1,
                ime_action: 6,
                package: "com.example.app".to_owned(),
            })
        );
        assert_eq!(
            decode(br#"{"op":"blur"}"#).expect("blur"),
            GuestMessage::Blur
        );
    }

    #[test]
    fn unknown_operation_is_preserved() {
        assert_eq!(
            decode(br#"{"op":"future","value":1}"#).expect("unknown"),
            GuestMessage::Unknown {
                op: "future".to_owned(),
            }
        );
    }

    #[test]
    fn long_lines_are_rejected() {
        let bytes = vec![b'x'; MAX_LINE_BYTES + 1];
        assert!(matches!(decode(&bytes), Err(ProtocolError::LineTooLong)));

        let mut framed = bytes;
        framed.push(b'\n');
        let mut reader = BufReader::new(Cursor::new(framed));
        assert!(matches!(
            read_line(&mut reader),
            Err(ProtocolError::LineTooLong)
        ));
    }
}
