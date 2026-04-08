pub fn xor_string_literal(value: &str, key: u8) -> Vec<u8> {
    value
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(index, byte)| byte ^ key.wrapping_add(index as u8).wrapping_add(1))
        .collect()
}

pub fn decode_xor_string(bytes: &[u8], key: u8) -> String {
    bytes
        .iter()
        .enumerate()
        .map(|(index, byte)| char::from(byte ^ key.wrapping_add(index as u8).wrapping_add(1)))
        .collect()
}

pub fn emit_luau_xor_decoder(name: &str) -> String {
    format!(
        "local {name}=function(t,k)local o={{}}for i=1,#t do o[i]=string.char(bit32.bxor(t[i],(k+i)%256))end return table.concat(o)end\n"
    )
}

pub fn emit_luau_encoded_string_expr(value: &str, key: u8, decoder_name: &str) -> String {
    let encoded = xor_string_literal(value, key);
    let mut bytes = String::new();
    for (index, byte) in encoded.iter().enumerate() {
        if index > 0 {
            bytes.push(',');
        }
        bytes.push_str(&byte.to_string());
    }
    format!("{decoder_name}({{{bytes}}},{key})")
}
