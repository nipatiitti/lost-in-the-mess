use raptorq::{Encoder, Decoder, ObjectTransmissionInformation, EncodingPacket, PayloadId};

fn main() {
    let oti = ObjectTransmissionInformation::with_defaults(100_000, 1024);
    let mut encoder = Encoder::with_defaults(&vec![0u8; 100_000], 1024);
    let mut decoder = Decoder::new(oti);
    let packet = encoder.get(vec![EncodingPacket::new(PayloadId::new(0, 0), vec![0; 1024])]);
}
