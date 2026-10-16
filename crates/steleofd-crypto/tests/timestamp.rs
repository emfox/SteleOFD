//! Tests for the GB/T 20520 timestamp token reader and verifier. The tokens
//! are built here rather than in the shared test kit, because the kit depends
//! on this crate and a cycle is not worth a fixture.

use steleofd_crypto::{Sm2KeyPair, TimeStampToken, sm3};

/// One DER tag-length-value triple.
fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
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
fn oid(arcs: &[u64]) -> Vec<u8> {
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

/// A minimal SM2 certificate, as the signed fixture uses.
fn sm2_cert_der(public_key: &[u8]) -> Vec<u8> {
    let algorithm = tlv(
        0x30,
        &[
            oid(&[1, 2, 840, 10045, 2, 1]),
            oid(&[1, 2, 156, 10197, 1, 301]),
        ]
        .concat(),
    );
    let empty_name = tlv(0x30, &[]);
    let validity = tlv(
        0x30,
        &[tlv(0x17, b"260101000000Z"), tlv(0x17, b"991231235959Z")].concat(),
    );
    let mut bits = vec![0x00];
    bits.extend_from_slice(public_key);
    let spki = tlv(0x30, &[algorithm.clone(), tlv(0x03, &bits)].concat());
    let mut tbs = tlv(0xA0, &integer(2));
    tbs.extend(integer(1));
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

const SM3: &[u64] = &[1, 2, 156, 10197, 1, 401];
const SM2_SM3: &[u64] = &[1, 2, 156, 10197, 1, 501];
const TST_INFO: &[u64] = &[1, 2, 840, 113549, 1, 9, 16, 1, 4];

/// A DER INTEGER from a small non-negative value, minimal as the rules demand.
fn integer(value: u64) -> Vec<u8> {
    let be = value.to_be_bytes();
    let first = be.iter().position(|b| *b != 0).unwrap_or(be.len() - 1);
    let mut bytes = be[first..].to_vec();
    if bytes.is_empty() {
        bytes.push(0);
    }
    // A leading bit set would read as negative, so a zero byte pads it.
    if bytes[0] & 0x80 != 0 {
        bytes.insert(0, 0);
    }
    tlv(0x02, &bytes)
}

/// Build a `TSTInfo` binding `imprint`.
fn tst_info(imprint: &[u8], serial: u64) -> Vec<u8> {
    let algorithm = tlv(0x30, &oid(SM3));
    let message_imprint = tlv(0x30, &[algorithm, tlv(0x04, imprint)].concat());
    tlv(
        0x30,
        &[
            integer(1),
            oid(&[1, 2, 156, 10197, 1, 1]),
            message_imprint,
            integer(serial),
            tlv(0x18, b"20260101000000Z"),
        ]
        .concat(),
    )
}

/// Wrap a signed `TSTInfo` into a CMS `ContentInfo`, certificate included.
/// `signed_attrs` decides whether the signature covers the TSTInfo directly or
/// a retagged attribute set, which is the branch RFC 5652 makes.
fn token(imprint: &[u8], serial: u64, pair: &Sm2KeyPair, signed_attrs: bool) -> Vec<u8> {
    let info = tst_info(imprint, serial);
    let (signed_data, attrs) = if signed_attrs {
        let attrs = tlv(0xA0, &tlv(0x30, &oid(SM3)));
        (attrs.clone(), Some(attrs))
    } else {
        (info.clone(), None)
    };
    let covered: Vec<u8> = match &attrs {
        Some(a) => {
            let mut set = vec![0x31u8];
            set.extend_from_slice(&a[1..]);
            set
        }
        None => signed_data.clone(),
    };
    let signature = pair.sign(&covered);
    let encap = tlv(
        0x30,
        &[oid(TST_INFO), tlv(0xA0, &tlv(0x04, &info))].concat(),
    );
    let digest_algorithms = tlv(0x31, &tlv(0x30, &oid(SM3)));
    let sid = tlv(0x30, &[tlv(0x30, &[]), integer(1)].concat());
    let mut signer_info = vec![integer(1), sid, tlv(0x30, &oid(SM3))];
    if let Some(a) = &attrs {
        signer_info.push(a.clone());
    }
    signer_info.push(tlv(0x30, &oid(SM2_SM3)));
    signer_info.push(tlv(0x04, &signature));
    let certificates = tlv(0xA0, &sm2_cert_der(&pair.public_bytes()));
    let signed = tlv(
        0x30,
        &[
            integer(3),
            digest_algorithms,
            encap,
            certificates,
            tlv(0x31, &tlv(0x30, &signer_info.concat())),
        ]
        .concat(),
    );
    tlv(0x30, &[oid(TST_INFO), tlv(0xA0, &signed)].concat())
}

#[test]
fn a_token_reads_its_time_policy_and_imprint() {
    let pair = Sm2KeyPair::generate().expect("keygen");
    let imprint = sm3(b"the signature value").to_vec();
    let bytes = token(&imprint, 7, &pair, false);
    let parsed = TimeStampToken::parse_der(&bytes).expect("a real token parses");
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.policy, "1.2.156.10197.1.1");
    assert_eq!(parsed.gen_time, "20260101000000Z");
    assert_eq!(parsed.serial_number, "7");
    assert_eq!(parsed.imprint, imprint);
    assert_eq!(parsed.imprint_algorithm, "1.2.156.10197.1.401");
    assert!(parsed.is_complete());
}

#[test]
fn a_token_keeps_its_tsa_certificate() {
    let pair = Sm2KeyPair::generate().expect("keygen");
    let bytes = token(&sm3(b"x"), 1, &pair, false);
    let parsed = TimeStampToken::parse_der(&bytes).expect("parse");
    let cert = parsed.certificate.expect("the token embeds a certificate");
    let subject = steleofd_crypto::Certificate::parse_der(&cert).expect("cert parses");
    assert_eq!(subject.public_key(), pair.public_bytes());
}

#[test]
fn the_tsa_signature_verifies_against_its_own_key() {
    let pair = Sm2KeyPair::generate().expect("keygen");
    let bytes = token(&sm3(b"proof"), 1, &pair, false);
    let parsed = TimeStampToken::parse_der(&bytes).expect("parse");
    assert!(
        parsed
            .verify_tsa_signature(&pair.public_bytes())
            .expect("well-formed"),
        "a token the TSA signed must verify against the TSA key"
    );
}

#[test]
fn a_foreign_key_does_not_verify_the_tsa_signature() {
    let pair = Sm2KeyPair::generate().expect("keygen");
    let other = Sm2KeyPair::generate().expect("keygen");
    let bytes = token(&sm3(b"proof"), 1, &pair, false);
    let parsed = TimeStampToken::parse_der(&bytes).expect("parse");
    assert!(
        !parsed
            .verify_tsa_signature(&other.public_bytes())
            .expect("well-formed"),
        "the signature must not pass with someone else's key"
    );
}

#[test]
fn a_token_signed_over_attributes_still_verifies() {
    let pair = Sm2KeyPair::generate().expect("keygen");
    let bytes = token(&sm3(b"proof"), 1, &pair, true);
    let parsed = TimeStampToken::parse_der(&bytes).expect("parse");
    assert!(
        parsed.signed_attrs.is_some(),
        "the attribute set is the signed data when present"
    );
    assert!(
        parsed
            .verify_tsa_signature(&pair.public_bytes())
            .expect("well-formed"),
        "the RFC 5652 attribute branch must verify too"
    );
}

#[test]
fn the_imprint_binds_the_message_it_timestamps() {
    let pair = Sm2KeyPair::generate().expect("keygen");
    let message = b"the signature value";
    let bytes = token(&sm3(message), 1, &pair, false);
    let parsed = TimeStampToken::parse_der(&bytes).expect("parse");
    assert!(parsed.covers(message), "the imprint is the message digest");
    assert!(
        !parsed.covers(b"a different signature value"),
        "a token must not claim another message"
    );
}

#[test]
fn a_tampered_imprint_fails_the_message_check() {
    let pair = Sm2KeyPair::generate().expect("keygen");
    let mut imprint = sm3(b"the signature value").to_vec();
    imprint[0] ^= 0xff;
    let bytes = token(&imprint, 1, &pair, false);
    let parsed = TimeStampToken::parse_der(&bytes).expect("parse");
    assert!(!parsed.covers(b"the signature value"));
}

#[test]
fn garbage_is_refused_rather_than_guessed_at() {
    assert!(TimeStampToken::parse_der(b"not a token").is_err());
    assert!(TimeStampToken::parse_der(&[]).is_err());
}

#[test]
fn a_certificate_only_token_is_incomplete() {
    // A ContentInfo with a different content type is refused outright.
    let foreign = tlv(0x30, &[oid(&[1, 2, 840, 113549, 1, 7, 1])].concat());
    assert!(TimeStampToken::parse_der(&foreign).is_err());
}
