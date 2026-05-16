pub struct FecFrame {
    pub object_id: u32,
    pub oti: [u8; 12],
    pub esi: u32,
    pub sym_sz: u16,
    pub payload: Vec<u8>,
}

impl FecFrame {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(22 + self.payload.len());
        buf.extend_from_slice(&self.object_id.to_be_bytes());
        buf.extend_from_slice(&self.oti);
        buf.extend_from_slice(&self.esi.to_be_bytes());
        buf.extend_from_slice(&self.sym_sz.to_be_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 22 {
            return None;
        }
        let object_id = u32::from_be_bytes(bytes[0..4].try_into().unwrap());
        let mut oti = [0u8; 12];
        oti.copy_from_slice(&bytes[4..16]);
        let esi = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let sym_sz = u16::from_be_bytes(bytes[20..22].try_into().unwrap());
        let payload = bytes[22..].to_vec();

        Some(FecFrame {
            object_id,
            oti,
            esi,
            sym_sz,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_roundtrip() {
        let oti = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02];
        let frame = FecFrame {
            object_id: 0xCAFEBABE,
            oti,
            esi: 42,
            sym_sz: 1024,
            payload: payload.clone(),
        };

        let encoded = frame.encode();
        let decoded = FecFrame::decode(&encoded).expect("decode should succeed");

        assert_eq!(decoded.object_id, 0xCAFEBABE);
        assert_eq!(decoded.oti, oti);
        assert_eq!(decoded.esi, 42);
        assert_eq!(decoded.sym_sz, 1024);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn test_frame_too_short() {
        assert!(FecFrame::decode(&[0u8; 21]).is_none());
        assert!(FecFrame::decode(&[]).is_none());
    }

    #[test]
    fn test_frame_empty_payload() {
        let frame = FecFrame {
            object_id: 1,
            oti: [0u8; 12],
            esi: 0,
            sym_sz: 512,
            payload: vec![],
        };
        let decoded = FecFrame::decode(&frame.encode()).expect("decode should succeed");
        assert_eq!(decoded.payload, Vec::<u8>::new());
    }
}
