//! Wire frame: 18-byte plaintext header (also AAD) + AEAD payload.
//!
//!  0   1   2          6          10                 18
//!  +---+---+----------+----------+------------------+----------------+
//!  |ver|flg|  epoch   |   sid    |     counter      | ct + tag (16B) |
//!  | 1 | 1 |    4 BE  |    4 BE  |        8 BE      |    variable    |
//!  +---+---+----------+----------+------------------+----------------+

use litm_common::{Epoch, Error, NodeId, PROTOCOL_VERSION, Result};

pub const HEADER_LEN: usize = 18;

#[derive(Copy, Clone, Debug)]
pub struct Header {
    pub version: u8,
    pub flags: u8,
    pub epoch: Epoch,
    pub sender: NodeId,
    pub counter: u64,
}

impl Header {
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        if out.len() < HEADER_LEN {
            return Err(Error::BadFrame("header buffer too small"));
        }
        out[0] = self.version;
        out[1] = self.flags;
        out[2..6].copy_from_slice(&self.epoch.to_be_bytes());
        out[6..10].copy_from_slice(&self.sender.to_be_bytes());
        out[10..18].copy_from_slice(&self.counter.to_be_bytes());
        Ok(())
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        if buf.len() < HEADER_LEN {
            return Err(Error::BadFrame("frame shorter than header"));
        }
        if buf[0] != PROTOCOL_VERSION {
            return Err(Error::BadFrame("protocol version mismatch"));
        }
        Ok(Self {
            version: buf[0],
            flags: buf[1],
            epoch: u32::from_be_bytes(buf[2..6].try_into().unwrap()),
            sender: u32::from_be_bytes(buf[6..10].try_into().unwrap()),
            counter: u64::from_be_bytes(buf[10..18].try_into().unwrap()),
        })
    }

    /// 12-byte AEAD nonce: sid(4 BE) || counter(8 BE). Globally unique per sender.
    pub fn nonce(&self) -> [u8; 12] {
        let mut n = [0u8; 12];
        n[0..4].copy_from_slice(&self.sender.to_be_bytes());
        n[4..12].copy_from_slice(&self.counter.to_be_bytes());
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let h = Header {
            version: PROTOCOL_VERSION,
            flags: 0,
            epoch: 7,
            sender: 42,
            counter: 0xdead_beef,
        };
        let mut buf = [0u8; HEADER_LEN];
        h.encode(&mut buf).unwrap();
        let d = Header::decode(&buf).unwrap();
        assert_eq!(d.epoch, 7);
        assert_eq!(d.sender, 42);
        assert_eq!(d.counter, 0xdead_beef);
    }

    #[test]
    fn nonce_layout() {
        let h = Header {
            version: 1,
            flags: 0,
            epoch: 0,
            sender: 0x01020304,
            counter: 0x05060708090a0b0c,
        };
        assert_eq!(h.nonce(), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    }

    #[test]
    fn wrong_version_rejected() {
        let mut buf = [0u8; HEADER_LEN];
        buf[0] = 99;
        assert!(Header::decode(&buf).is_err());
    }
}
