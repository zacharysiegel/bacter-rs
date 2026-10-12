use std::fmt::Write;

pub fn encode_hexadecimal(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");

        text
    })
}

/// `None` for an odd length or a character outside hexadecimal.
pub fn decode_hexadecimal(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }

    let every_character_is_hexadecimal: bool = text.bytes().all(|byte| byte.is_ascii_hexdigit());

    if !every_character_is_hexadecimal {
        return None;
    }

    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(text.get(index..index + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_hexadecimal_round_trips_encode_hexadecimal() {
        let bytes: Vec<u8> = vec![0x00, 0x01, 0x7f, 0x80, 0xab, 0xff];
        let text: String = encode_hexadecimal(&bytes);

        assert_eq!(text, "00017f80abff");
        assert_eq!(decode_hexadecimal(&text), Some(bytes));
    }

    #[test]
    fn decode_hexadecimal_rejects_an_odd_length() {
        assert_eq!(decode_hexadecimal("abc"), None);
    }

    #[test]
    fn decode_hexadecimal_rejects_an_invalid_character() {
        assert_eq!(decode_hexadecimal("0g"), None);
    }

    #[test]
    fn decode_hexadecimal_rejects_a_sign() {
        assert_eq!(decode_hexadecimal("+f"), None);
    }
}
