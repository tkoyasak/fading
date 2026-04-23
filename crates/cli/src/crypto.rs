use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};

/// Encrypt `plaintext` with AES-256-GCM.
/// `key_hex` must be exactly 64 lowercase hex characters (32 bytes).
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
/// `key_hex` must be exactly 64 lowercase hex characters (32 bytes).
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
    let mut key_bytes = [0u8; 32];
    base16ct::lower::decode(key_hex.trim(), &mut key_bytes)
        .context("FADING_CLI_ENCRYPTION_KEY must be exactly 64 lowercase hex characters")?;
    Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|_| anyhow::anyhow!("FADING_CLI_ENCRYPTION_KEY must be exactly 32 bytes"))
}
