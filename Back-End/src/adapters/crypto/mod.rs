//! Hash de senha e cifra/decifra de arquivos.

use aes_gcm::aead::rand_core::{OsRng, RngCore};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroize;

use crate::domain::{Dek, Erro, WrappedDek};
use crate::ports::Crypto;

const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 16;

#[derive(Clone, Copy)]
pub struct AesGcmCrypto;

fn derive_kek(password: &str, salt: &[u8]) -> Result<[u8; 32], Erro> {
    let mut kek = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default())
        .hash_password_into(password.as_bytes(), salt, &mut kek)
        .map_err(|_| Erro::Interno)?;
    Ok(kek)
}

fn encrypt_with(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, Erro> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| Erro::Interno)?;

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

fn decrypt_with(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>, Erro> {
    if blob.len() < NONCE_LEN {
        return Err(Erro::Interno);
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(&blob[..NONCE_LEN]);
    cipher
        .decrypt(nonce, &blob[NONCE_LEN..])
        .map_err(|_| Erro::Interno)
}

impl Crypto for AesGcmCrypto {
    fn generate_dek(&self) -> Dek {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Dek::new(bytes)
    }

    fn wrap_dek(&self, password: &str, dek: &Dek) -> Result<WrappedDek, Erro> {
        let mut salt = vec![0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        let mut kek = derive_kek(password, &salt)?;
        let blob = encrypt_with(&kek, dek.as_bytes());
        kek.zeroize();
        Ok(WrappedDek { salt, blob: blob? })
    }

    fn unwrap_dek(&self, password: &str, wrapped: &WrappedDek) -> Result<Dek, Erro> {
        let mut kek = derive_kek(password, &wrapped.salt)?;
        let bytes = decrypt_with(&kek, &wrapped.blob);
        kek.zeroize();
        let bytes = bytes?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| Erro::Interno)?;
        Ok(Dek::new(arr))
    }

    fn encrypt(&self, dek: &Dek, plaintext: &[u8]) -> Result<Vec<u8>, Erro> {
        encrypt_with(dek.as_bytes(), plaintext)
    }

    fn decrypt(&self, dek: &Dek, ciphertext: &[u8]) -> Result<Vec<u8>, Erro> {
        decrypt_with(dek.as_bytes(), ciphertext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_unwrap_roundtrip() {
        let crypto = AesGcmCrypto;
        let dek = crypto.generate_dek();
        let wrapped = crypto.wrap_dek("secret12", &dek).unwrap();
        let out = crypto.unwrap_dek("secret12", &wrapped).unwrap();
        assert_eq!(dek.as_bytes(), out.as_bytes());
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let crypto = AesGcmCrypto;
        let dek = crypto.generate_dek();
        let blob = crypto.encrypt(&dek, b"hello vault").unwrap();
        let plain = crypto.decrypt(&dek, &blob).unwrap();
        assert_eq!(plain, b"hello vault");
    }

    #[test]
    fn wrong_password_does_not_unwrap() {
        let crypto = AesGcmCrypto;
        let dek = crypto.generate_dek();
        let wrapped = crypto.wrap_dek("secret12", &dek).unwrap();
        assert!(crypto.unwrap_dek("wrongpass", &wrapped).is_err());
    }
}
