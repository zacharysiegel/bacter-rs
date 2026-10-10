use shared::error::AppError;

const BASE64_PADDING: u8 = b'=';
const BASE64_BITS_PER_SYMBOL: u32 = 6;
const BITS_PER_BYTE: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameEncodingKind {
    Binary,
    Hex,
    Base64,
}

/// ASCII whitespace in hex and base64 input is ignored.
pub fn decode_frame_input(input_bytes: &[u8], encoding: FrameEncodingKind) -> Result<Vec<u8>, AppError> {
    match encoding {
        FrameEncodingKind::Binary => Ok(input_bytes.to_vec()),
        FrameEncodingKind::Hex => decode_hex(&get_symbols(input_bytes)),
        FrameEncodingKind::Base64 => decode_base64(&get_symbols(input_bytes)),
    }
}

fn get_symbols(input_bytes: &[u8]) -> Vec<u8> {
    input_bytes.iter().copied().filter(|byte| !byte.is_ascii_whitespace()).collect()
}

fn decode_hex(symbols: &[u8]) -> Result<Vec<u8>, AppError> {
    if symbols.len() % 2 != 0 {
        return Err(AppError::new("hex input has an odd number of digits"));
    }

    symbols
        .chunks(2)
        .map(|digit_pair| Ok(get_hex_value(digit_pair[0])? << 4 | get_hex_value(digit_pair[1])?))
        .collect()
}

fn get_hex_value(symbol: u8) -> Result<u8, AppError> {
    let Some(value): Option<u32> = char::from(symbol).to_digit(16) else {
        return Err(AppError::new(&format!("{:?} is not a hex digit", char::from(symbol))));
    };

    Ok(u8::try_from(value)?)
}

/// The standard alphabet; padding is optional.
fn decode_base64(symbols: &[u8]) -> Result<Vec<u8>, AppError> {
    let unpadded_symbols: &[u8] = symbols.strip_suffix(&[BASE64_PADDING, BASE64_PADDING]).unwrap_or(symbols);
    let unpadded_symbols: &[u8] = unpadded_symbols.strip_suffix(&[BASE64_PADDING]).unwrap_or(unpadded_symbols);

    if unpadded_symbols.len() % 4 == 1 {
        return Err(AppError::new("base64 input does not encode whole bytes"));
    }

    let mut bytes: Vec<u8> = Vec::new();
    let mut accumulated_bits: u32 = 0;
    let mut accumulated_bit_count: u32 = 0;

    for symbol in unpadded_symbols {
        accumulated_bits = (accumulated_bits << BASE64_BITS_PER_SYMBOL) | get_base64_value(*symbol)?;
        accumulated_bit_count += BASE64_BITS_PER_SYMBOL;

        if accumulated_bit_count >= BITS_PER_BYTE {
            accumulated_bit_count -= BITS_PER_BYTE;
            bytes.push(u8::try_from((accumulated_bits >> accumulated_bit_count) & 0xff)?);
            accumulated_bits &= (1 << accumulated_bit_count) - 1;
        }
    }

    Ok(bytes)
}

fn get_base64_value(symbol: u8) -> Result<u32, AppError> {
    let value: u8 = match symbol {
        b'A'..=b'Z' => symbol - b'A',
        b'a'..=b'z' => symbol - b'a' + 26,
        b'0'..=b'9' => symbol - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => {
            return Err(AppError::new(&format!(
                "{:?} is not a base64 symbol",
                char::from(symbol)
            )));
        }
    };

    Ok(u32::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_frame_input_keeps_binary_input() {
        assert_eq!(
            decode_frame_input(&[0, 10, 255], FrameEncodingKind::Binary).unwrap(),
            vec![0, 10, 255]
        );
    }

    #[test]
    fn decode_frame_input_reads_hex_and_ignores_whitespace() {
        assert_eq!(
            decode_frame_input(b"00 0a\nFF", FrameEncodingKind::Hex).unwrap(),
            vec![0, 10, 255]
        );
    }

    #[test]
    fn decode_frame_input_rejects_odd_or_invalid_hex() {
        assert!(decode_frame_input(b"0a0", FrameEncodingKind::Hex).is_err());
        assert!(decode_frame_input(b"0g", FrameEncodingKind::Hex).is_err());
    }

    #[test]
    fn decode_frame_input_reads_base64_with_and_without_padding() {
        assert_eq!(
            decode_frame_input(b"Zm9vYmFy", FrameEncodingKind::Base64).unwrap(),
            b"foobar"
        );
        assert_eq!(
            decode_frame_input(b"Zm9vYg==\n", FrameEncodingKind::Base64).unwrap(),
            b"foob"
        );
        assert_eq!(
            decode_frame_input(b"Zm9vYmE", FrameEncodingKind::Base64).unwrap(),
            b"fooba"
        );
        assert_eq!(
            decode_frame_input(b"+/8=", FrameEncodingKind::Base64).unwrap(),
            vec![0xfb, 0xff]
        );
    }

    #[test]
    fn decode_frame_input_rejects_invalid_base64() {
        assert!(decode_frame_input(b"Zm9v!", FrameEncodingKind::Base64).is_err());
        assert!(decode_frame_input(b"Zm9vY", FrameEncodingKind::Base64).is_err());
    }
}
