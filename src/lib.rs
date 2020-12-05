//! Elliptic Curve Integrated Encryption Scheme for secp256k1 in Rust, based on [pure Rust implementation](https://github.com/paritytech/libsecp256k1) of secp256k1.
//!
//! ECIES functionalities are built upon AES-GCM-256 and HKDF-SHA256.
//!
//! This is the Rust version of [eciespy](https://github.com/ecies/py).
//!
//! # Quick Start
//!
//! ```rust
//! use ecies::{decrypt, encrypt, utils::generate_keypair};
//!
//! const MSG: &str = "helloworld";
//! let (sk, pk) = generate_keypair();
//! let (sk, pk) = (&sk.serialize(), &pk.serialize());
//!
//! let msg = MSG.as_bytes();
//! assert_eq!(
//!     msg,
//!     decrypt(sk, &encrypt(pk, msg).unwrap()).unwrap().as_slice()
//! );
//! ```
//!
//! # Optional pure Rust AES backend
//!
//! You can choose to use OpenSSL implementation or [pure Rust implementation](https://github.com/RustCrypto/AEADs) of AES-256-GCM:
//!
//! ```toml
//! # ecies = {version = "0.2", feature = "openssl"}
//! ecies = {version = "0.2", feature = "pure"}
//! ```
//!
//! Due to some [performance problem](https://github.com/RustCrypto/AEADs/issues/243), OpenSSL is the default backend.
//!
//! Pure Rust implementation is sometimes useful, such as building a WASM target: `cargo build --no-default-features --features pure --target=wasm32-unknown-unknown`.

pub use secp256k1::{util::FULL_PUBLIC_KEY_SIZE, Error as SecpError, PublicKey, SecretKey};

/// Constant variables
pub mod consts;
/// Type aliases
pub mod types;
/// Utility functions for ecies
pub mod utils;

#[cfg(feature = "openssl")]
mod openssl_aes;
#[cfg(feature = "pure")]
mod pure_aes;

use utils::{aes_decrypt, aes_encrypt, decapsulate, encapsulate, generate_keypair};

/// Encrypt a message by a public key
///
/// # Arguments
///
/// * `receiver_pub` - The u8 array reference of a receiver's public key
/// * `msg` - The u8 array reference of the message to encrypt
pub fn encrypt(receiver_pub: &[u8], msg: &[u8]) -> Result<Vec<u8>, SecpError> {
    let receiver_pk = PublicKey::parse_slice(receiver_pub, None)?;
    let (ephemeral_sk, ephemeral_pk) = generate_keypair();

    let aes_key = encapsulate(&ephemeral_sk, &receiver_pk);
    let encrypted = aes_encrypt(&aes_key, msg).ok_or(SecpError::InvalidMessage)?;

    let mut cipher_text = Vec::with_capacity(FULL_PUBLIC_KEY_SIZE + encrypted.len());
    cipher_text.extend(ephemeral_pk.serialize().iter());
    cipher_text.extend(encrypted);

    Ok(cipher_text)
}

/// Decrypt a message by a secret key
///
/// # Arguments
///
/// * `receiver_sec` - The u8 array reference of a receiver's secret key
/// * `msg` - The u8 array reference of the encrypted message
pub fn decrypt(receiver_sec: &[u8], msg: &[u8]) -> Result<Vec<u8>, SecpError> {
    let receiver_sk = SecretKey::parse_slice(receiver_sec)?;

    if msg.len() < FULL_PUBLIC_KEY_SIZE {
        return Err(SecpError::InvalidMessage);
    }

    let ephemeral_pk = PublicKey::parse_slice(&msg[..FULL_PUBLIC_KEY_SIZE], None)?;
    let encrypted = &msg[FULL_PUBLIC_KEY_SIZE..];

    let aes_key = decapsulate(&ephemeral_pk, &receiver_sk);

    aes_decrypt(&aes_key, encrypted).ok_or(SecpError::InvalidMessage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex::encode;
    use utils::{decode_hex, generate_keypair};

    const PYTHON_BACKEND: &str = "https://eciespy.herokuapp.com/";
    const MSG: &str = "helloworld";

    const BIG_MSG_SIZE: usize = 2 * 1024 * 1024; // 2 MB
    const BIG_MSG: [u8; BIG_MSG_SIZE] = [1u8; BIG_MSG_SIZE];

    fn test_enc_dec(sk: &[u8], pk: &[u8]) {
        let msg = MSG.as_bytes();
        assert_eq!(msg, decrypt(sk, &encrypt(pk, msg).unwrap()).unwrap().as_slice());

        let msg = &BIG_MSG;
        assert_eq!(msg.to_vec(), decrypt(sk, &encrypt(pk, msg).unwrap()).unwrap());
    }

    #[test]
    fn attempts_to_decrypt_with_another_key() {
        let (_, pk1) = generate_keypair();

        let (sk2, _) = generate_keypair();

        assert_eq!(
            decrypt(
                &sk2.serialize(),
                encrypt(&pk1.serialize_compressed(), b"text").unwrap().as_slice()
            ),
            Err(SecpError::InvalidMessage)
        );
    }

    #[test]
    fn attempts_to_decrypt_incorrect_message() {
        let (sk, _) = generate_keypair();

        assert_eq!(decrypt(&sk.serialize(), &[]), Err(SecpError::InvalidMessage));

        assert_eq!(decrypt(&sk.serialize(), &[0u8; 65]), Err(SecpError::InvalidPublicKey));
    }

    #[test]
    fn attempts_to_encrypt_with_invalid_key() {
        assert_eq!(encrypt(&[0u8; 33], b"text"), Err(SecpError::InvalidPublicKey));
    }

    #[test]
    fn test_compressed_public() {
        let (sk, pk) = generate_keypair();
        let (sk, pk) = (&sk.serialize(), &pk.serialize_compressed());
        test_enc_dec(sk, pk);
    }

    #[test]
    fn test_uncompressed_public() {
        let (sk, pk) = generate_keypair();
        let (sk, pk) = (&sk.serialize(), &pk.serialize());
        test_enc_dec(sk, pk);
    }

    #[test]
    fn test_against_python() {
        use futures_util::FutureExt;
        use tokio::runtime::Runtime;

        let (sk, pk) = generate_keypair();

        let sk_hex = encode(&sk.serialize().to_vec());
        let uncompressed_pk = &pk.serialize();
        let pk_hex = encode(uncompressed_pk.to_vec());

        let client = reqwest::Client::new();
        let params = [("data", MSG), ("pub", pk_hex.as_str())];

        let mut rt = Runtime::new().unwrap();
        let res = rt
            .block_on(
                client
                    .post(PYTHON_BACKEND)
                    .form(&params)
                    .send()
                    .then(|r| r.unwrap().text()),
            )
            .unwrap();

        let server_encrypted = decode_hex(&res);
        let local_decrypted = decrypt(&sk.serialize(), server_encrypted.as_slice()).unwrap();
        assert_eq!(local_decrypted, MSG.as_bytes());

        let local_encrypted = encrypt(uncompressed_pk, MSG.as_bytes()).unwrap();
        let params = [("data", encode(local_encrypted)), ("prv", sk_hex)];

        let res = rt
            .block_on(
                client
                    .post(PYTHON_BACKEND)
                    .form(&params)
                    .send()
                    .then(|r| r.unwrap().text()),
            )
            .unwrap();

        assert_eq!(res, MSG);
    }
}
