//! SM4 block cipher (GB/T 32907-2016) in ECB and CBC modes with PKCS#7 padding.

use cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit, array::Array};
use steleofd_error::{CryptoError, OfdError};

/// Block size of SM4 in bytes.
pub const SM4_BLOCK_LEN: usize = 16;

/// Key length of SM4 in bytes.
pub const SM4_KEY_LEN: usize = 16;

/// Chaining mode of an SM4 operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sm4Mode {
    /// Electronic codebook: blocks are independent.
    Ecb,
    /// Cipher block chaining: each block is XORed with the previous one.
    Cbc,
}

/// An SM4 key bound to a chaining mode.
#[derive(Debug, Clone)]
pub struct Sm4Key {
    key: [u8; SM4_KEY_LEN],
    mode: Sm4Mode,
}

impl Sm4Key {
    /// Build a key from 16 bytes and a chaining mode.
    pub fn new(key: &[u8], mode: Sm4Mode) -> Result<Self, OfdError> {
        let bytes: [u8; SM4_KEY_LEN] = key.try_into().map_err(|_| {
            OfdError::Crypto(CryptoError::InvalidKey(format!(
                "SM4 key must be {SM4_KEY_LEN} bytes, got {}",
                key.len()
            )))
        })?;
        Ok(Self { key: bytes, mode })
    }

    /// The chaining mode this key operates in.
    pub fn mode(&self) -> Sm4Mode {
        self.mode
    }

    /// Encrypt plaintext; CBC additionally needs a 16-byte IV.
    pub fn encrypt(&self, plaintext: &[u8], iv: Option<&[u8]>) -> Result<Vec<u8>, OfdError> {
        let iv = self.check_iv(iv)?;
        let padded = pkcs7_pad(plaintext);
        let mut out = padded;
        self.apply(&mut out, iv, Direction::Encrypt)?;
        Ok(out)
    }

    /// Decrypt ciphertext; CBC additionally needs a 16-byte IV.
    pub fn decrypt(&self, ciphertext: &[u8], iv: Option<&[u8]>) -> Result<Vec<u8>, OfdError> {
        if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(SM4_BLOCK_LEN) {
            return Err(OfdError::Crypto(CryptoError::Operation(format!(
                "ciphertext length must be a positive multiple of {SM4_BLOCK_LEN}"
            ))));
        }
        let iv = self.check_iv(iv)?;
        let mut buf = ciphertext.to_vec();
        self.apply(&mut buf, iv, Direction::Decrypt)?;
        pkcs7_unpad(&buf)
    }

    fn check_iv(&self, iv: Option<&[u8]>) -> Result<Option<[u8; SM4_BLOCK_LEN]>, OfdError> {
        match (self.mode, iv) {
            (Sm4Mode::Ecb, _) => Ok(None),
            (Sm4Mode::Cbc, Some(bytes)) => {
                let block: [u8; SM4_BLOCK_LEN] = bytes.try_into().map_err(|_| {
                    OfdError::Crypto(CryptoError::InvalidKey(format!(
                        "CBC IV must be {SM4_BLOCK_LEN} bytes"
                    )))
                })?;
                Ok(Some(block))
            }
            (Sm4Mode::Cbc, None) => Err(OfdError::Crypto(CryptoError::InvalidKey(
                "CBC mode requires a 16-byte IV".into(),
            ))),
        }
    }

    fn apply(
        &self,
        buf: &mut [u8],
        iv: Option<[u8; SM4_BLOCK_LEN]>,
        dir: Direction,
    ) -> Result<(), OfdError> {
        let cipher = sm4::Sm4::new_from_slice(&self.key)
            .map_err(|e| OfdError::Crypto(CryptoError::Operation(e.to_string())))?;
        match self.mode {
            Sm4Mode::Ecb => {
                for block in buf.chunks_exact_mut(SM4_BLOCK_LEN) {
                    apply_block(&cipher, block, dir);
                }
            }
            Sm4Mode::Cbc => {
                let mut prev = iv.expect("checked above");
                for block in buf.chunks_exact_mut(SM4_BLOCK_LEN) {
                    match dir {
                        Direction::Encrypt => {
                            for (b, p) in block.iter_mut().zip(prev.iter()) {
                                *b ^= p;
                            }
                            apply_block(&cipher, block, dir);
                            prev.copy_from_slice(block);
                        }
                        Direction::Decrypt => {
                            let mut tmp = [0u8; SM4_BLOCK_LEN];
                            tmp.copy_from_slice(block);
                            apply_block(&cipher, block, dir);
                            for (b, p) in block.iter_mut().zip(prev.iter()) {
                                *b ^= p;
                            }
                            prev = tmp;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    Encrypt,
    Decrypt,
}

fn apply_block(cipher: &sm4::Sm4, block: &mut [u8], dir: Direction) {
    let mut arr: cipher::Block<sm4::Sm4> = Array::default();
    arr.copy_from_slice(block);
    match dir {
        Direction::Encrypt => cipher.encrypt_block(&mut arr),
        Direction::Decrypt => cipher.decrypt_block(&mut arr),
    }
    block.copy_from_slice(&arr);
}

fn pkcs7_pad(data: &[u8]) -> Vec<u8> {
    let pad = (SM4_BLOCK_LEN - data.len() % SM4_BLOCK_LEN) as u8;
    let mut out = data.to_vec();
    out.extend(std::iter::repeat_n(pad, pad as usize));
    out
}

fn pkcs7_unpad(data: &[u8]) -> Result<Vec<u8>, OfdError> {
    let pad = match data.last() {
        Some(b) if *b >= 1 && *b as usize <= SM4_BLOCK_LEN && *b as usize <= data.len() => *b,
        _ => return Err(OfdError::Crypto(CryptoError::DecryptFailed)),
    };
    if data[data.len() - pad as usize..].iter().any(|b| *b != pad) {
        return Err(OfdError::Crypto(CryptoError::DecryptFailed));
    }
    Ok(data[..data.len() - pad as usize].to_vec())
}
