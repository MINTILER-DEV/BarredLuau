use barred_luau::config::EncoderConfig;
use barred_luau::serializer::{EncoderKey, decode, encode};

#[test]
fn custom_encoder_roundtrips() {
    let cfg = EncoderConfig::default();
    let key = EncoderKey {
        seed: 0x1234_5678,
        nonce: 0x8765_4321,
    };
    let payload = b"local value = 1337";
    let encoded = encode(payload, &key, &cfg).expect("encoding should succeed");
    let decoded = decode(&encoded, &key, &cfg).expect("decoding should succeed");
    assert_eq!(decoded, payload);
}

#[test]
fn encoder_seed_changes_output() {
    let cfg = EncoderConfig::default();
    let payload = b"same payload";
    let first = encode(payload, &EncoderKey { seed: 1, nonce: 2 }, &cfg).expect("first encoding");
    let second = encode(payload, &EncoderKey { seed: 3, nonce: 4 }, &cfg).expect("second encoding");
    assert_ne!(first, second);
}

#[test]
fn encoder_checksum_detects_tampering() {
    let cfg = EncoderConfig::default();
    let key = EncoderKey {
        seed: 99,
        nonce: 42,
    };
    let encoded = encode(b"tamper me", &key, &cfg).expect("encoding should succeed");
    let mut chars: Vec<char> = encoded.chars().collect();
    let replacement = if chars[0] == 'q' { '7' } else { 'q' };
    chars[0] = replacement;
    let tampered: String = chars.into_iter().collect();
    let error = decode(&tampered, &key, &cfg).expect_err("tampering should fail");
    assert!(matches!(
        error,
        barred_luau::CompileError::Integrity(_) | barred_luau::CompileError::Decode(_)
    ));
}
