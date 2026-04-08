pub fn xor_string_literal(value: &str, key: u8) -> Vec<u8> {
    value.as_bytes().iter().map(|byte| byte ^ key).collect()
}

pub fn decode_xor_string(bytes: &[u8], key: u8) -> String {
    bytes.iter().map(|byte| char::from(byte ^ key)).collect()
}
