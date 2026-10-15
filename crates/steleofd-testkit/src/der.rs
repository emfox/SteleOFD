//! Just enough DER and base64 to build an SM2 certificate a parser accepts.
//! The engine reads a certificate out of a signature, so the tests need real
//! DER rather than a stub, and hand-rolling it avoids an encoder dependency.

/// Base64, the encoding `CheckValue` and `Cert` use on the wire.
pub fn base64_of(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let mut group = [0u8; 3];
        group[..chunk.len()].copy_from_slice(chunk);
        let bits = u32::from_be_bytes([group[0], group[1], group[2], 0]);
        out.push(TABLE[(bits >> 26) as usize & 0x3f] as char);
        out.push(TABLE[(bits >> 20) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(bits >> 14) as usize & 0x3f] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(bits >> 8) as usize & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

/// One DER tag-length-value triple.
pub fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let len = content.len();
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let be = len.to_be_bytes();
        let first = be.iter().position(|b| *b != 0).unwrap_or(be.len() - 1);
        out.push((be.len() - first) as u8 | 0x80);
        out.extend_from_slice(&be[first..]);
    }
    out.extend_from_slice(content);
    out
}

/// A DER object identifier from its arcs.
pub fn oid(arcs: &[u64]) -> Vec<u8> {
    let mut content = vec![(arcs[0] * 40 + arcs[1]) as u8];
    for &arc in &arcs[2..] {
        let mut stack = vec![(arc & 0x7f) as u8];
        let mut rest = arc >> 7;
        while rest > 0 {
            stack.push(((rest & 0x7f) as u8) | 0x80);
            rest >>= 7;
        }
        stack.reverse();
        content.extend(stack);
    }
    tlv(0x06, &content)
}

/// A minimal X.509 certificate carrying one SM2 public key: just enough
/// structure for a parser to read the SubjectPublicKeyInfo back out.
pub fn sm2_cert_der(public_key: &[u8]) -> Vec<u8> {
    let ec_public_key = oid(&[1, 2, 840, 10045, 2, 1]);
    let sm2_curve = oid(&[1, 2, 156, 10197, 1, 301]);
    let algorithm = tlv(0x30, &[ec_public_key, sm2_curve].concat());
    let empty_name = tlv(0x30, &[]);
    let validity = tlv(
        0x30,
        &[tlv(0x17, b"260101000000Z"), tlv(0x17, b"991231235959Z")].concat(),
    );
    let mut bit_string = vec![0x00];
    bit_string.extend_from_slice(public_key);
    let spki = tlv(0x30, &[algorithm.clone(), tlv(0x03, &bit_string)].concat());

    let mut tbs = tlv(0xA0, &tlv(0x02, &[0x02]));
    tbs.extend(tlv(0x02, &[0x01]));
    tbs.extend(&algorithm);
    tbs.extend(&empty_name);
    tbs.extend(&validity);
    tbs.extend(&empty_name);
    tbs.extend(&spki);

    let mut cert = tlv(0x30, &tbs);
    cert.extend(&algorithm);
    cert.extend(tlv(0x03, &[0x00]));
    tlv(0x30, &cert)
}
