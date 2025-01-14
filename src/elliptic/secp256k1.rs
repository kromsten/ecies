use libsecp256k1::{PublicKey, SecretKey};
use rand_core::RngCore;

use crate::compat::Vec;
use crate::config::is_hkdf_key_compressed;
use crate::consts::SharedSecret;
use crate::symmetric::hkdf_derive;

pub use libsecp256k1::Error;

/// Generate a `(SecretKey, PublicKey)` pair
pub fn generate_keypair(rng: &mut impl RngCore) -> (SecretKey, PublicKey) {
    let sk = SecretKey::random(rng);
    let pk = PublicKey::from_secret_key(&sk);
    (sk, pk)
}

/// Calculate a shared symmetric key of our secret key and peer's public key by hkdf
pub fn encapsulate(sk: &SecretKey, peer_pk: &PublicKey) -> Result<SharedSecret, Error> {
    let mut shared_point = *peer_pk;
    shared_point.tweak_mul_assign(sk)?;
    let sender_point = &PublicKey::from_secret_key(sk);
    Ok(get_shared_secret(sender_point, &shared_point))
}

/// Calculate a shared symmetric key of our public key and peer's secret key by hkdf
pub fn decapsulate(pk: &PublicKey, peer_sk: &SecretKey) -> Result<SharedSecret, Error> {
    let mut shared_point = *pk;
    shared_point.tweak_mul_assign(peer_sk)?;
    Ok(get_shared_secret(pk, &shared_point))
}

/// Parse secret key bytes
pub fn parse_sk(sk: &[u8]) -> Result<SecretKey, Error> {
    SecretKey::parse_slice(sk)
}

/// Parse public key bytes
pub fn parse_pk(pk: &[u8]) -> Result<PublicKey, Error> {
    PublicKey::parse_slice(pk, None)
}

/// Public key to bytes
pub fn pk_to_vec(pk: &PublicKey, compressed: bool) -> Vec<u8> {
    if compressed {
        pk.serialize_compressed().to_vec()
    } else {
        pk.serialize().to_vec()
    }
}

fn get_shared_secret(sender_point: &PublicKey, shared_point: &PublicKey) -> SharedSecret {
    if is_hkdf_key_compressed() {
        hkdf_derive(
            &sender_point.serialize_compressed(),
            &shared_point.serialize_compressed(),
        )
    } else {
        hkdf_derive(&sender_point.serialize(), &shared_point.serialize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::tests::decode_hex;

    #[test]
    fn test_secret_validity() {
        // 0 < private key < group order is valid
        let mut zero = [0u8; 32];
        let group_order = decode_hex("0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141");
        let invalid_sks = [zero.to_vec(), group_order];

        for sk in invalid_sks.iter() {
            assert_eq!(parse_sk(sk).err().unwrap(), Error::InvalidSecretKey);
        }

        zero[31] = 1;

        let one = zero;
        let group_order_minus_1 = decode_hex("0Xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140");
        let valid_sks = [one.to_vec(), group_order_minus_1];
        for sk in valid_sks.iter() {
            parse_sk(sk).unwrap();
        }
    }

   

    /// Generate two secret keys with values 2 and 3
    pub fn get_sk2_sk3() -> (SecretKey, SecretKey) {
        let mut two = [0u8; 32];
        let mut three = [0u8; 32];
        two[31] = 2u8;
        three[31] = 3u8;

        let sk2 = SecretKey::parse_slice(&two).unwrap();
        let sk3 = SecretKey::parse_slice(&three).unwrap();
        (sk2, sk3)
    }

    #[test]
    pub fn test_known_shared_secret() {
        let (sk2, sk3) = get_sk2_sk3();
        let pk3 = PublicKey::from_secret_key(&sk3);

        assert_eq!(
            encapsulate(&sk2, &pk3).unwrap().to_vec(),
            decode_hex("6f982d63e8590c9d9b5b4c1959ff80315d772edd8f60287c9361d548d5200f82")
        );
    }
}

#[cfg(test)]
mod lib_tests {
    use super::Error;
    use crate::decrypt;

    const MSG: &str = "helloworld🌍";
    const BIG_MSG_SIZE: usize = 2 * 1024 * 1024; // 2 MB
    const BIG_MSG: [u8; BIG_MSG_SIZE] = [1u8; BIG_MSG_SIZE];

 
    #[test]
    pub fn attempts_to_decrypt_with_invalid_key() {
        assert_eq!(decrypt(&[0u8; 32], &[]), Err(Error::InvalidSecretKey));
    }


}

#[cfg(test)]
mod config_tests {
    use super::*;

    use crate::config::{reset_config, update_config, Config};
    use crate::utils::tests::decode_hex;
    use tests::get_sk2_sk3;

    const MSG: &str = "helloworld🌍";

    #[test]
    pub fn test_known_hkdf_config() {
        let (sk2, sk3) = get_sk2_sk3();
        let pk3 = PublicKey::from_secret_key(&sk3);

        update_config(Config {
            is_hkdf_key_compressed: true,
            ..Config::default()
        });

        let encapsulated = encapsulate(&sk2, &pk3).unwrap();

        assert_eq!(
            encapsulated.to_vec(),
            decode_hex("b192b226edb3f02da11ef9c6ce4afe1c7e40be304e05ae3b988f4834b1cb6c69")
        );

        reset_config();
    }

 
}

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test() {
        super::tests::test_known_shared_secret();
    }

    #[wasm_bindgen_test]
    fn test_config() {
        super::config_tests::test_ephemeral_key_config();
        super::config_tests::test_known_hkdf_config();
    }

    #[wasm_bindgen_test]
    fn test_lib() {
        super::lib_tests::test_compressed_public();
        super::lib_tests::test_uncompressed_public();
    }

    #[wasm_bindgen_test]
    fn test_error() {
        super::lib_tests::attempts_to_encrypt_with_invalid_key();
        super::lib_tests::attempts_to_decrypt_with_invalid_key();
        super::lib_tests::attempts_to_decrypt_incorrect_message();
        super::lib_tests::attempts_to_decrypt_with_another_key();
    }
}
