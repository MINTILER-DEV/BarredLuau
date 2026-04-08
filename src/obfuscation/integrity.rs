use crate::serializer::checksum::fnv1a32;

pub fn checksum_label(label: &str, payload: &[u8]) -> u32 {
    let mut bytes = Vec::with_capacity(label.len() + payload.len());
    bytes.extend_from_slice(label.as_bytes());
    bytes.extend_from_slice(payload);
    fnv1a32(&bytes)
}
