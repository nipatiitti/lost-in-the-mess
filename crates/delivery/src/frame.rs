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
