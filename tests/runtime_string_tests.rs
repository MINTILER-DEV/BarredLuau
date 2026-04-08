use barred_luau::obfuscation::literal_encoder::{decode_xor_string, xor_string_literal};

#[test]
fn runtime_string_xor_roundtrips() {
    let original = "barredluau integrity check failed";
    let key = 248;
    let encoded = xor_string_literal(original, key);
    let decoded = decode_xor_string(&encoded, key);
    assert_eq!(decoded, original);
}
