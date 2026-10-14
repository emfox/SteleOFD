//! Tests for steleofd-crypto primitives against official vectors.

use steleofd_crypto::{Certificate, Sm2KeyPair, Sm3Hash, Sm4Key, Sm4Mode, sm2_verify, sm3};

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex digit"))
        .collect()
}

#[test]
fn sm3_matches_official_short_vector() {
    let got = sm3(b"abc");
    let want = hex("66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0");
    assert_eq!(got.to_vec(), want);
}

#[test]
fn sm3_matches_official_long_vector() {
    let msg: Vec<u8> = b"abcd".repeat(16);
    let got = sm3(&msg);
    let want = hex("debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732");
    assert_eq!(got.to_vec(), want);
}

#[test]
fn sm3_incremental_equals_one_shot() {
    let data = vec![7u8; 1000];
    let mut streamed = Sm3Hash::new();
    streamed.update(&data[..333]);
    streamed.update(&data[333..]);
    assert_eq!(streamed.finish().to_vec(), sm3(&data).to_vec());
}

#[test]
fn sm4_ecb_matches_official_vector() {
    let key = hex("0123456789abcdeffedcba9876543210");
    let plain = hex("0123456789abcdeffedcba9876543210");
    let want = hex("681edf34d206965e86b3e94f536e4246");
    let k = Sm4Key::new(&key, Sm4Mode::Ecb).expect("key");
    let got = k.encrypt(&plain, None).expect("encrypt");
    assert_eq!(&got[..16], &want[..], "first block is the raw ciphertext");
    assert_eq!(got.len(), 32, "PKCS#7 appends a full padding block");
    assert_eq!(k.decrypt(&got, None).expect("decrypt"), plain);
}

#[test]
fn sm4_cbc_roundtrip_with_chaining() {
    let key = [5u8; 16];
    let iv = [9u8; 16];
    let k = Sm4Key::new(&key, Sm4Mode::Cbc).expect("key");
    let plain = b"attack at dawn!!".to_vec();
    let ct = k.encrypt(&plain, Some(&iv)).expect("encrypt");
    assert_ne!(ct, plain);
    assert_eq!(k.decrypt(&ct, Some(&iv)).expect("decrypt"), plain);
}

#[test]
fn sm4_cbc_iv_changes_ciphertext() {
    let k = Sm4Key::new(&[1u8; 16], Sm4Mode::Cbc).expect("key");
    let a = k.encrypt(b"sixteen bytes...", Some(&[0u8; 16])).expect("a");
    let b = k.encrypt(b"sixteen bytes...", Some(&[1u8; 16])).expect("b");
    assert_ne!(a, b);
}

#[test]
fn sm4_rejects_bad_key_and_iv() {
    assert!(Sm4Key::new(&[0u8; 15], Sm4Mode::Ecb).is_err());
    let k = Sm4Key::new(&[0u8; 16], Sm4Mode::Cbc).expect("key");
    assert!(k.encrypt(b"x", None).is_err());
    assert!(k.encrypt(b"x", Some(&[0u8; 8])).is_err());
}

#[test]
fn sm4_decrypt_detects_corrupted_padding() {
    let k = Sm4Key::new(&[3u8; 16], Sm4Mode::Ecb).expect("key");
    let ct = k.encrypt(b"hello", None).expect("encrypt");
    let mut broken = ct.clone();
    broken[0] ^= 0xff;
    let err = k.decrypt(&broken, None).expect_err("must fail");
    assert!(err.to_string().contains("decryption failed"));
}

#[test]
fn sm2_roundtrip_sign_and_verify() {
    let pair = Sm2KeyPair::generate().expect("keygen");
    let msg = b"steleofd signature test";
    let sig = pair.sign(msg);
    assert!(sm2_verify(&pair.public_bytes(), msg, &sig).expect("verify"));
}

#[test]
fn sm2_rejects_tampered_message_and_signature() {
    let pair = Sm2KeyPair::generate().expect("keygen");
    let sig = pair.sign(b"original message");
    assert!(!sm2_verify(&pair.public_bytes(), b"tampered message", &sig).expect("verify"));
    let mut broken = sig.clone();
    broken[10] ^= 0x01;
    let ok = sm2_verify(&pair.public_bytes(), b"original message", &broken);
    assert!(ok.is_err() || !ok.expect("parseable"));
}

#[test]
fn sm2_secret_bytes_reproduce_public_key() {
    let pair = Sm2KeyPair::generate().expect("keygen");
    let again = Sm2KeyPair::from_secret_bytes(&pair.secret_bytes()).expect("restore");
    assert_eq!(pair.public_bytes(), again.public_bytes());
    assert_eq!(pair.public_bytes().len(), 65);
    assert_eq!(pair.public_bytes()[0], 0x04);
}

#[test]
fn sm2_rejects_malformed_inputs() {
    assert!(Sm2KeyPair::from_secret_bytes(&[1u8; 4]).is_err());
    assert!(sm2_verify(&[0xff; 65], b"m", b"").is_err());
}

#[test]
fn certificate_parses_der_and_pem() {
    let der = include_bytes!("data/test-cert.der");
    let cert = Certificate::parse_der(der).expect("der");
    assert_eq!(cert.subject(), "CN=SteleOFD Test, O=SteleOFD");
    assert_eq!(cert.subject(), cert.issuer(), "self-signed fixture");
    assert_eq!(
        cert.serial_hex(),
        "61802e01fd816a30608c4cd2066e5c98342f2ff6"
    );
    let pem = include_str!("data/test-cert.crt.txt");
    let from_pem = Certificate::parse_pem(pem).expect("pem");
    assert_eq!(cert.raw_der(), from_pem.raw_der());
}

#[test]
fn certificate_expiry_and_self_signed_checks() {
    let der = include_bytes!("data/test-cert.der");
    let cert = Certificate::parse_der(der).expect("der");
    let mid = (cert.not_before() + cert.not_after()) / 2;
    assert!(!cert.is_expired_at(mid));
    assert!(cert.is_expired_at(cert.not_before() - 1));
    assert!(cert.is_expired_at(cert.not_after() + 1));
    assert!(cert.is_self_signed());
}

#[test]
fn certificate_rejects_garbage() {
    assert!(Certificate::parse_der(b"not a certificate").is_err());
    assert!(Certificate::parse_pem("hello world").is_err());
    assert!(
        Certificate::parse_pem("-----BEGIN CERTIFICATE-----\n!!!!\n-----END CERTIFICATE-----")
            .is_err()
    );
}
