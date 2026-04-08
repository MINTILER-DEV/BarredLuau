use std::collections::{BTreeMap, BTreeSet};

use crate::config::EncoderConfig;
use crate::error::CompileError;
use crate::serializer::checksum::fnv1a32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncoderKey {
    pub seed: u32,
    pub nonce: u32,
}

#[derive(Clone, Debug)]
struct Lcg {
    state: u32,
}

impl Lcg {
    fn new(seed: u32) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        self.state
    }

    fn next_u8(&mut self) -> u8 {
        (self.next_u32() >> 24) as u8
    }

    fn bounded(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u32() as usize) % bound
        }
    }
}

pub fn encode(bytes: &[u8], key: &EncoderKey, cfg: &EncoderConfig) -> Result<String, CompileError> {
    validate_config(cfg)?;
    let mut payload = frame_payload(bytes, cfg.include_checksum);
    if cfg.interleave {
        payload = interleave(&payload);
    }
    for round in 0..cfg.rounds {
        let round_seed = round_seed(key, round as u32);
        let permutation = build_permutation(payload.len(), round_seed ^ 0xA17C_91E3);
        payload = apply_permutation(&payload, &permutation);
        apply_stream_transform(&mut payload, round_seed ^ 0xC0DE_7705, true);
        let substitution = build_substitution_table(round_seed ^ 0x55AA_10F1);
        substitute(&mut payload, &substitution);
    }
    encode_text(&payload, cfg)
}

pub fn decode(text: &str, key: &EncoderKey, cfg: &EncoderConfig) -> Result<Vec<u8>, CompileError> {
    validate_config(cfg)?;
    let mut payload = decode_text(text, cfg)?;
    for round in (0..cfg.rounds).rev() {
        let round_seed = round_seed(key, round as u32);
        let substitution = build_substitution_table(round_seed ^ 0x55AA_10F1);
        let inverse = inverse_substitution_table(&substitution);
        substitute(&mut payload, &inverse);
        apply_stream_transform(&mut payload, round_seed ^ 0xC0DE_7705, false);
        let permutation = build_permutation(payload.len(), round_seed ^ 0xA17C_91E3);
        payload = invert_permutation(&payload, &permutation);
    }
    if cfg.interleave {
        payload = deinterleave(&payload);
    }
    deframe_payload(&payload, cfg.include_checksum)
}

fn validate_config(cfg: &EncoderConfig) -> Result<(), CompileError> {
    let alphabet_len = cfg.alphabet.chars().count();
    if alphabet_len < 16 {
        return Err(CompileError::Config(
            "Encoder alphabet must contain at least 16 unique symbols".to_string(),
        ));
    }
    if alphabet_len * alphabet_len <= 255 {
        return Err(CompileError::Config(
            "Encoder alphabet is too small for the custom radix packing".to_string(),
        ));
    }
    let unique: BTreeSet<char> = cfg.alphabet.chars().collect();
    if unique.len() != alphabet_len {
        return Err(CompileError::Config(
            "Encoder alphabet must not contain duplicate characters".to_string(),
        ));
    }
    if cfg.alphabet.contains(':') {
        return Err(CompileError::Config(
            "Encoder alphabet must not contain the chunk separator ':'".to_string(),
        ));
    }
    Ok(())
}

fn frame_payload(bytes: &[u8], include_checksum: bool) -> Vec<u8> {
    let mut framed = Vec::with_capacity(bytes.len() + 8);
    framed.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    if include_checksum {
        framed.extend_from_slice(&fnv1a32(bytes).to_le_bytes());
    }
    framed.extend_from_slice(bytes);
    framed
}

fn deframe_payload(bytes: &[u8], include_checksum: bool) -> Result<Vec<u8>, CompileError> {
    let header_len = if include_checksum { 8 } else { 4 };
    if bytes.len() < header_len {
        return Err(CompileError::Decode(
            "Encoded payload was shorter than the framing header".to_string(),
        ));
    }
    let length = u32::from_le_bytes(bytes[..4].try_into().expect("length header")) as usize;
    let checksum = if include_checksum {
        Some(u32::from_le_bytes(
            bytes[4..8].try_into().expect("checksum header"),
        ))
    } else {
        None
    };
    let payload = bytes
        .get(header_len..header_len + length)
        .ok_or_else(|| CompileError::Decode("Encoded payload length was truncated".to_string()))?;
    if bytes.len() != header_len + length {
        return Err(CompileError::Decode(
            "Encoded payload had trailing or missing bytes".to_string(),
        ));
    }
    if let Some(expected) = checksum {
        let actual = fnv1a32(payload);
        if expected != actual {
            return Err(CompileError::Integrity(
                "Custom encoder checksum verification failed".to_string(),
            ));
        }
    }
    Ok(payload.to_vec())
}

fn round_seed(key: &EncoderKey, round: u32) -> u32 {
    key.seed
        .wrapping_add(key.nonce.rotate_left(5))
        .wrapping_add(round.wrapping_mul(977))
        ^ 0x9E37_79B9
}

fn build_permutation(len: usize, seed: u32) -> Vec<usize> {
    let mut permutation: Vec<usize> = (0..len).collect();
    let mut prng = Lcg::new(seed);
    for index in (1..len).rev() {
        let target = prng.bounded(index + 1);
        permutation.swap(index, target);
    }
    permutation
}

fn apply_permutation(bytes: &[u8], permutation: &[usize]) -> Vec<u8> {
    permutation.iter().map(|index| bytes[*index]).collect()
}

fn invert_permutation(bytes: &[u8], permutation: &[usize]) -> Vec<u8> {
    let mut restored = vec![0u8; bytes.len()];
    for (output_index, source_index) in permutation.iter().enumerate() {
        restored[*source_index] = bytes[output_index];
    }
    restored
}

fn apply_stream_transform(bytes: &mut [u8], seed: u32, encode: bool) {
    let mut prng = Lcg::new(seed);
    for byte in bytes {
        let add = prng.next_u8();
        let mask = prng.next_u8();
        if encode {
            *byte = byte.wrapping_add(add) ^ mask;
        } else {
            *byte = (*byte ^ mask).wrapping_sub(add);
        }
    }
}

fn build_substitution_table(seed: u32) -> [u8; 256] {
    let mut table = [0u8; 256];
    for (index, value) in table.iter_mut().enumerate() {
        *value = index as u8;
    }
    let mut prng = Lcg::new(seed);
    for index in (1..table.len()).rev() {
        let target = prng.bounded(index + 1);
        table.swap(index, target);
    }
    table
}

fn inverse_substitution_table(table: &[u8; 256]) -> [u8; 256] {
    let mut inverse = [0u8; 256];
    for (index, value) in table.iter().enumerate() {
        inverse[*value as usize] = index as u8;
    }
    inverse
}

fn substitute(bytes: &mut [u8], table: &[u8; 256]) {
    for byte in bytes {
        *byte = table[*byte as usize];
    }
}

fn encode_text(bytes: &[u8], cfg: &EncoderConfig) -> Result<String, CompileError> {
    let alphabet: Vec<char> = cfg.alphabet.chars().collect();
    let radix = alphabet.len();
    let mut output = String::with_capacity(bytes.len() * 2);
    for (index, byte) in bytes.iter().enumerate() {
        let high = (*byte as usize) / radix;
        let low = (*byte as usize) % radix;
        output.push(alphabet[high]);
        output.push(alphabet[low]);
        if cfg.chunk_size > 0 && (index + 1) % cfg.chunk_size == 0 && index + 1 != bytes.len() {
            output.push(':');
        }
    }
    Ok(output)
}

fn decode_text(text: &str, cfg: &EncoderConfig) -> Result<Vec<u8>, CompileError> {
    let alphabet: Vec<char> = cfg.alphabet.chars().collect();
    let radix = alphabet.len();
    let reverse: BTreeMap<char, usize> = alphabet
        .iter()
        .enumerate()
        .map(|(index, ch)| (*ch, index))
        .collect();
    let mut digits = Vec::new();
    for ch in text.chars() {
        if ch == ':' {
            continue;
        }
        let digit = reverse.get(&ch).copied().ok_or_else(|| {
            CompileError::Decode(format!(
                "Character `{ch}` is not present in the custom alphabet"
            ))
        })?;
        digits.push(digit);
    }
    if digits.len() % 2 != 0 {
        return Err(CompileError::Decode(
            "Custom alphabet payload had an uneven number of digits".to_string(),
        ));
    }
    let mut bytes = Vec::with_capacity(digits.len() / 2);
    for chunk in digits.chunks_exact(2) {
        let value = chunk[0] * radix + chunk[1];
        if value > u8::MAX as usize {
            return Err(CompileError::Decode(
                "Custom radix pair decoded outside the byte range".to_string(),
            ));
        }
        bytes.push(value as u8);
    }
    Ok(bytes)
}

fn interleave(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    out.extend(bytes.iter().step_by(2).copied());
    out.extend(bytes.iter().skip(1).step_by(2).copied());
    out
}

fn deinterleave(bytes: &[u8]) -> Vec<u8> {
    let left_len = bytes.len().div_ceil(2);
    let (left, right) = bytes.split_at(left_len);
    let mut out = Vec::with_capacity(bytes.len());
    for index in 0..left_len {
        out.push(left[index]);
        if let Some(byte) = right.get(index) {
            out.push(*byte);
        }
    }
    out
}
