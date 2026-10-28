use crate::domain::{Dek, Erro, WrappedDek};

pub trait Crypto: Send + Sync {
    fn generate_dek(&self) -> Dek;
    fn wrap_dek(&self, password: &str, dek: &Dek) -> Result<WrappedDek, Erro>;
    fn unwrap_dek(&self, password: &str, wrapped: &WrappedDek) -> Result<Dek, Erro>;
    fn encrypt(&self, dek: &Dek, plaintext: &[u8]) -> Result<Vec<u8>, Erro>;
    fn decrypt(&self, dek: &Dek, ciphertext: &[u8]) -> Result<Vec<u8>, Erro>;
}
