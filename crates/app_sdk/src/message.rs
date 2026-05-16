use litm_common::{NodeId, ObjectId};
use serde::{Deserialize, Serialize};

use crate::error::{Result, SdkError};

pub const SDK_ENVELOPE_VERSION: u8 = 0x01;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum VideoCodec {
    Mjpeg,
    H264,
    Raw,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MessagePayload {
    Text { content: String },
    File { name: String, bytes: Vec<u8> },
    Image { mime: String, bytes: Vec<u8> },
    VideoFrame { seq: u32, width: u16, height: u16, codec: VideoCodec, data: Vec<u8> },
    /// Escape hatch for application-specific types not yet in the enum.
    Custom { tag: String, bytes: Vec<u8> },
}

#[derive(Clone, Debug)]
pub struct ReceivedMessage {
    pub id: ObjectId,
    pub source: NodeId,
    pub payload: MessagePayload,
}

pub fn encode_message(payload: &MessagePayload) -> Result<Vec<u8>> {
    let kind_byte: u8 = match payload {
        MessagePayload::Text { .. } => 0,
        MessagePayload::File { .. } => 1,
        MessagePayload::Image { .. } => 2,
        MessagePayload::VideoFrame { .. } => 3,
        MessagePayload::Custom { .. } => 4,
    };
    let mut out = vec![SDK_ENVELOPE_VERSION, kind_byte];
    let body = postcard::to_allocvec(payload).map_err(|e| SdkError::Serde(e.to_string()))?;
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn decode_message(bytes: &[u8]) -> Result<MessagePayload> {
    if bytes.len() < 2 {
        return Err(SdkError::Serde("envelope too short".into()));
    }
    if bytes[0] != SDK_ENVELOPE_VERSION {
        return Err(SdkError::Serde(format!("unknown SDK version {}", bytes[0])));
    }
    postcard::from_bytes::<MessagePayload>(&bytes[2..]).map_err(|e| SdkError::Serde(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_roundtrip() {
        let msg = MessagePayload::Text { content: "hello mesh".into() };
        let bytes = encode_message(&msg).unwrap();
        assert_eq!(bytes[0], SDK_ENVELOPE_VERSION);
        assert_eq!(bytes[1], 0); // Text kind byte
        let decoded = decode_message(&bytes).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn file_roundtrip() {
        let msg = MessagePayload::File { name: "data.bin".into(), bytes: vec![1, 2, 3, 4] };
        let bytes = encode_message(&msg).unwrap();
        assert_eq!(decode_message(&bytes).unwrap(), msg);
    }

    #[test]
    fn video_frame_roundtrip() {
        let msg = MessagePayload::VideoFrame {
            seq: 42,
            width: 640,
            height: 480,
            codec: VideoCodec::Mjpeg,
            data: vec![0xFF, 0xD8],
        };
        let bytes = encode_message(&msg).unwrap();
        assert_eq!(bytes[1], 3); // VideoFrame kind byte
        assert_eq!(decode_message(&bytes).unwrap(), msg);
    }

    #[test]
    fn custom_roundtrip() {
        let msg = MessagePayload::Custom { tag: "my-app/v1".into(), bytes: vec![0xDE, 0xAD] };
        assert_eq!(decode_message(&encode_message(&msg).unwrap()).unwrap(), msg);
    }

    #[test]
    fn wrong_version_rejected() {
        let mut bytes = encode_message(&MessagePayload::Text { content: "x".into() }).unwrap();
        bytes[0] = 0xFF;
        assert!(matches!(decode_message(&bytes), Err(SdkError::Serde(_))));
    }

    #[test]
    fn too_short_rejected() {
        assert!(matches!(decode_message(&[0x01]), Err(SdkError::Serde(_))));
        assert!(matches!(decode_message(&[]), Err(SdkError::Serde(_))));
    }
}
