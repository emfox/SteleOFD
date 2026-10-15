//! Minimal base64 encoding, for SVG data URIs.
//! The project keeps its own codecs rather than pulling a crate for twenty
//! lines of table lookup, matching the decoders in crypto and sign.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode bytes as standard base64, padded to a multiple of four characters.
pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(encode(&[]), "");
    }

    #[test]
    fn one_byte_pads_with_two_equals() {
        assert_eq!(encode(&[0x00]), "AA==");
        assert_eq!(encode(&[0xff]), "/w==");
    }

    #[test]
    fn two_bytes_pad_with_one_equals() {
        assert_eq!(encode(&[0x00, 0x00]), "AAA=");
        assert_eq!(encode(&[0xff, 0xff]), "//8=");
    }

    #[test]
    fn three_bytes_need_no_padding() {
        assert_eq!(encode(&[0x00, 0x00, 0x00]), "AAAA");
        assert_eq!(encode(&[0xff, 0xff, 0xff]), "////");
    }

    #[test]
    fn longer_inputs_split_on_three_byte_boundaries() {
        assert_eq!(encode(&[1, 2, 3, 4]), "AQIDBA==");
        assert_eq!(encode(b"SteleOFD"), "U3RlbGVPRkQ=");
    }

    #[test]
    fn output_length_is_always_a_multiple_of_four() {
        for len in 0..64usize {
            let data: Vec<u8> = (0..len).map(|i| i as u8).collect();
            let encoded = encode(&data);
            assert_eq!(encoded.len() % 4, 0);
            assert_eq!(encoded.len(), len.div_ceil(3) * 4);
        }
    }
}
