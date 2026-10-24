//! SM3 message digest (GB/T 32905-2016).

use sm3::Digest;

/// The object identifier GM/T 0004 gives SM3, which a signature descriptor
/// names in `CheckMethod`. Producers also spell the algorithm `SM3`.
pub const SM3_OID: &str = "1.2.156.10197.1.401";

/// Length of an SM3 digest in bytes.
pub const SM3_DIGEST_LEN: usize = 32;

/// Compute the SM3 digest of a byte slice in one shot.
pub fn sm3(data: &[u8]) -> [u8; SM3_DIGEST_LEN] {
    let mut hasher = Sm3Hash::new();
    hasher.update(data);
    hasher.finish()
}

/// Incremental SM3 hasher for streaming input.
#[derive(Debug, Clone)]
pub struct Sm3Hash {
    inner: sm3::Sm3,
}

impl Default for Sm3Hash {
    fn default() -> Self {
        Self::new()
    }
}

impl Sm3Hash {
    /// Create a fresh hasher.
    pub fn new() -> Self {
        Self {
            inner: sm3::Sm3::new(),
        }
    }

    /// Feed more bytes into the digest.
    pub fn update(&mut self, data: &[u8]) {
        Digest::update(&mut self.inner, data);
    }

    /// Finalize and return the 32-byte digest.
    pub fn finish(&mut self) -> [u8; SM3_DIGEST_LEN] {
        self.inner.finalize_reset().into()
    }
}
