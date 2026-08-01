pub(crate) fn encode_component(value: &str) -> String {
    js_sys::encode_uri_component(value)
        .as_string()
        .unwrap_or_else(|| value.to_string())
}

pub(crate) fn base64_encode(input: &[u8]) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    let mut i = 0;
    while i < input.len() {
        let b0 = input[i];
        let b1 = if i + 1 < input.len() { input[i + 1] } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] } else { 0 };

        let triplet = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

        result.push(CHARSET[((triplet >> 18) & 0x3F) as usize] as char);
        result.push(CHARSET[((triplet >> 12) & 0x3F) as usize] as char);

        if i + 1 < input.len() {
            result.push(CHARSET[((triplet >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if i + 2 < input.len() {
            result.push(CHARSET[(triplet & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        i += 3;
    }
    result
}
