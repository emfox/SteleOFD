//! Cryptographic primitives for OFD: SM3 digest, SM4 block cipher, SM2
//! signatures and X.509 certificate inspection.
//!
//! PKCS#12 container loading is tracked as follow-up work.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod asymmetric;
pub mod cert;
pub mod digest;
pub mod symmetric;

pub use asymmetric::{Sm2KeyPair, sm2_verify};
pub use cert::Certificate;
pub use digest::{Sm3Hash, sm3};
pub use symmetric::{Sm4Key, Sm4Mode};
