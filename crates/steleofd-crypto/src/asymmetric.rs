//! SM2 asymmetric signatures (GB/T 32918-2016).

use signature::{Signer, Verifier};
use steleofd_error::{CryptoError, OfdError};

/// Length of an SM2 secret key in bytes.
pub const SM2_SECRET_LEN: usize = 32;

/// Default distinguishing identifier mandated by GB/T 32918 part 5.
pub const DEFAULT_DISTINGUISHED_ID: &str = "1234567812345678";

/// An SM2 signing key pair.
#[derive(Debug, Clone)]
pub struct Sm2KeyPair {
    signing: sm2::dsa::SigningKey,
}

impl Sm2KeyPair {
    /// Generate a fresh random key pair.
    pub fn generate() -> Result<Self, OfdError> {
        for _ in 0..8 {
            let mut seed = [0u8; SM2_SECRET_LEN];
            getrandom::fill(&mut seed)
                .map_err(|e| OfdError::Crypto(CryptoError::Operation(e.to_string())))?;
            if let Ok(pair) = Self::from_secret_bytes(&seed) {
                return Ok(pair);
            }
        }
        Err(OfdError::Crypto(CryptoError::Operation(
            "failed to draw a valid SM2 scalar".into(),
        )))
    }

    /// Rebuild a key pair from 32 secret key bytes.
    pub fn from_secret_bytes(bytes: &[u8]) -> Result<Self, OfdError> {
        let signing = sm2::dsa::SigningKey::from_slice(DEFAULT_DISTINGUISHED_ID, bytes)
            .map_err(|e| OfdError::Crypto(CryptoError::InvalidKey(e.to_string())))?;
        Ok(Self { signing })
    }

    /// The uncompressed public key (65 bytes starting with 0x04).
    pub fn public_bytes(&self) -> Vec<u8> {
        self.signing.verifying_key().to_sec1_bytes().into_vec()
    }

    /// The raw secret key bytes.
    pub fn secret_bytes(&self) -> Vec<u8> {
        self.signing.to_bytes().to_vec()
    }

    /// Sign a message with SM3 as the digest, returning a DER signature.
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        self.signing.sign(message).to_der().to_vec()
    }
}

/// Verify an SM2 signature over a message the signer already digested.
///
/// GB/T 38540 signs a digest rather than the bytes it summarises, so the
/// message arithmetic is `SM3(ZA || digest)` and the digest arrives as `M`.
///
/// Returns `Ok(true)` when valid, `Ok(false)` when well-formed but wrong.
pub fn sm2_verify(
    public_key: &[u8],
    message: &[u8],
    signature_der: &[u8],
) -> Result<bool, OfdError> {
    verify_with(
        public_key,
        |key, sig| key.verify(message, sig),
        signature_der,
    )
}

/// Verify a signature over a digest that was signed as a prehash: the caller
/// states the bytes the signer digested, and this one checks them without
/// hashing again.
pub fn sm2_verify_digest(
    public_key: &[u8],
    digest: &[u8],
    signature_der: &[u8],
) -> Result<bool, OfdError> {
    verify_with(
        public_key,
        |key, sig| {
            use signature::hazmat::PrehashVerifier;
            key.verify_prehash(digest, sig)
        },
        signature_der,
    )
}

/// Run one verification against a parsed key and signature.
fn verify_with(
    public_key: &[u8],
    check: impl FnOnce(&sm2::dsa::VerifyingKey, &sm2::dsa::Signature) -> Result<(), signature::Error>,
    signature_der: &[u8],
) -> Result<bool, OfdError> {
    let verifying = sm2::dsa::VerifyingKey::from_sec1_bytes(DEFAULT_DISTINGUISHED_ID, public_key)
        .map_err(|_| {
        OfdError::Crypto(CryptoError::InvalidKey("malformed public key".into()))
    })?;
    let sig = sm2::dsa::Signature::from_der(signature_der)
        .map_err(|_| OfdError::Crypto(CryptoError::Operation("malformed signature".into())))?;
    Ok(check(&verifying, &sig).is_ok())
}
