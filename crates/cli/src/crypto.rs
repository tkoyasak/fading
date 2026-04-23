use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};

/// Encrypt `plaintext` with AES-256-GCM.
/// `key_hex` must be exactly 64 hex characters (32 bytes).
/// Output format: `[nonce (12 B)][ciphertext + tag (16 B)]`.
pub(crate) fn encrypt(key_hex: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = build_cipher(key_hex)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

    let mut out = nonce.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt data produced by [`encrypt`].
/// `key_hex` must be exactly 64 hex characters (32 bytes).
pub(crate) fn decrypt(key_hex: &str, data: &[u8]) -> Result<Vec<u8>> {
    const NONCE_LEN: usize = 12;
    if data.len() < NONCE_LEN {
        anyhow::bail!("Data too short to contain a nonce");
    }

    let cipher = build_cipher(key_hex)?;
    let nonce = Nonce::from_slice(&data[..NONCE_LEN]);
    cipher
        .decrypt(nonce, &data[NONCE_LEN..])
        .map_err(|_| anyhow::anyhow!("Decryption failed (wrong key or corrupted data)"))
}

fn build_cipher(key_hex: &str) -> Result<Aes256Gcm> {
    let hex = key_hex.trim();
    anyhow::ensure!(
        hex.len() == 64,
        "FADING_CLI_ENCRYPTION_KEY must be exactly 64 hex characters"
    );
    let mut key_bytes = [0u8; 32];
    base16ct::mixed::decode(hex, &mut key_bytes)
        .context("FADING_CLI_ENCRYPTION_KEY must be exactly 64 hex characters")?;
    Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|_| anyhow::anyhow!("FADING_CLI_ENCRYPTION_KEY must be exactly 32 bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn roundtrip() {
        let plaintext = b"hello, fading!";
        let ciphertext = encrypt(KEY, plaintext).unwrap();
        let recovered = decrypt(KEY, &ciphertext).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn roundtrip_empty() {
        let ciphertext = encrypt(KEY, b"").unwrap();
        let recovered = decrypt(KEY, &ciphertext).unwrap();
        assert_eq!(recovered, b"");
    }

    #[test]
    fn bad_key_length() {
        assert!(encrypt("deadbeef", b"data").is_err());
        assert!(decrypt("deadbeef", &[0u8; 28]).is_err());
    }

    #[test]
    fn data_too_short() {
        // data shorter than NONCE_LEN (12 B) should error before decryption
        assert!(decrypt(KEY, &[0u8; 11]).is_err());
    }

    #[test]
    fn wrong_key_fails() {
        let other_key = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        let ciphertext = encrypt(KEY, b"secret").unwrap();
        assert!(decrypt(other_key, &ciphertext).is_err());
    }

    #[test]
    fn uppercase_hex_key_accepted() {
        let upper = KEY.to_uppercase();
        let ciphertext = encrypt(&upper, b"hello").unwrap();
        let recovered = decrypt(&upper, &ciphertext).unwrap();
        assert_eq!(recovered, b"hello");
    }
}
