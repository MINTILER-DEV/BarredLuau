pub fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash = 0x811C_9DC5u32;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

pub fn seal_metadata(header: &[u8], payload: &[u8], feature_flags: u32) -> u32 {
    let mut bytes = Vec::with_capacity(header.len() + payload.len() + std::mem::size_of::<u32>());
    bytes.extend_from_slice(header);
    bytes.extend_from_slice(payload);
    bytes.extend_from_slice(&feature_flags.to_le_bytes());
    fnv1a32(&bytes)
}
