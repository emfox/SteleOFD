#![no_main]

//! A signature carries its signer's certificate, and that certificate comes
//! from whoever signed. Reading it must end in a value or an error, and the
//! accessors a path check calls must work on whatever parsed.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(cert) = sofd_crypto::Certificate::parse_der(data) else {
        return;
    };
    let _ = cert.subject();
    let _ = cert.issuer();
    let _ = cert.serial_hex();
    let _ = cert.not_before();
    let _ = cert.not_after();
    let _ = cert.is_expired_at(0);
    let _ = cert.is_ca();
    let _ = cert.path_len();
    let _ = cert.key_usage();
    let _ = cert.is_self_signed();
    let _ = cert.subject_key_id();
    let _ = cert.authority_key_id();
    // Verifying with its own key is the cheapest way to reach the signature
    // comparison, which is where a malformed BIT STRING would surface.
    let _ = cert.verify_signature(cert.public_key());
});
