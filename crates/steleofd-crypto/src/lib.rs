//! Cryptographic primitives for OFD: SM3 digest, SM4 block cipher, SM2
//! signatures, X.509 certificate inspection and GB/T 20520 timestamps.
//! PKCS#12 is follow-up work.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod asymmetric;
pub mod cert;
pub mod digest;
pub mod symmetric;
pub mod timestamp;

pub use asymmetric::{Sm2KeyPair, sm2_verify};
pub use cert::Certificate;
pub use digest::{Sm3Hash, sm3};
pub use symmetric::{Sm4Key, Sm4Mode};
pub use timestamp::TimeStampToken;
