pub(crate) fn encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(byte) {
            encoded.push(*byte as char);
        } else {
            encoded.push('%');
            encoded.push(hex_digit(byte >> 4));
            encoded.push(hex_digit(byte & 0x0f));
        }
    }
    encoded
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'A' + value - 10) as char,
        _ => unreachable!("hex digit is limited to four bits"),
    }
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

#[cfg(test)]
mod tests {
    use super::{base64_encode, encode_component};

    #[test]
    fn encodes_uri_component_without_browser_globals() {
        assert_eq!(encode_component("a b/c?"), "a%20b%2Fc%3F");
        assert_eq!(encode_component("safe-_.!~*'()"), "safe-_.!~*'()");
    }

    #[test]
    fn encodes_base64_boundaries() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
    }
}
