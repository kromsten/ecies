use once_cell::sync::OnceCell;

/// ECIES config. Make sure all parties use the same config
#[derive(Default, Debug, Clone)]
pub struct Config {
    pub is_ephemeral_key_compressed: bool,
    pub is_hkdf_key_compressed: bool,
}

/// Global config variable
pub static ECIES_CONFIG: OnceCell<Config> = OnceCell::new();

/// Update global config
pub fn update_config(config: Config) {
    ECIES_CONFIG.set(config).unwrap();
}

/// Reset global config to default
pub fn reset_config() {
    update_config(Config::default())
}

/// Get ephemeral key compressed or not
pub fn is_ephemeral_key_compressed() -> bool {
    ECIES_CONFIG.get_or_init(||
        Config::default()
    ).is_ephemeral_key_compressed
}

/// Get ephemeral key size: compressed(33) or uncompressed(65)
// #[cfg(feature = "secp256k1")]
pub fn get_ephemeral_key_size() -> usize {
    use crate::consts::{COMPRESSED_PUBLIC_KEY_SIZE, UNCOMPRESSED_PUBLIC_KEY_SIZE};

    if is_ephemeral_key_compressed() {
        COMPRESSED_PUBLIC_KEY_SIZE
    } else {
        UNCOMPRESSED_PUBLIC_KEY_SIZE
    }
}

// #[cfg(feature = "x25519")]
// pub fn get_ephemeral_key_size() -> usize {
//     32
// }

// #[cfg(all(not(feature = "x25519"), not(feature = "secp256k1")))]
// pub fn get_ephemeral_key_size() -> usize {
//     panic!("Not implemented")
// }

/// Get hkdf key derived from compressed shared point or not
pub fn is_hkdf_key_compressed() -> bool {
    ECIES_CONFIG.get_or_init(||
        Config::default()
    ).is_hkdf_key_compressed
}
