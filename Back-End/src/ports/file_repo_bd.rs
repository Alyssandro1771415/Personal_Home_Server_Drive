use std::future::Future;

use crate::domain::{Erro, FileId, StoredFile, UserId};

pub trait FileRepoBd: Send + Sync {
    fn create(
        &self,
        owner: UserId,
        nome: String,
        mime: Option<String>,
        tamanho_bytes: i64,
        storage_key: String,
    ) -> impl Future<Output = Result<StoredFile, Erro>> + Send;

    fn list_root(
        &self,
        owner: UserId,
    ) -> impl Future<Output = Result<Vec<StoredFile>, Erro>> + Send;

    fn get(&self, id: FileId) -> impl Future<Output = Result<Option<StoredFile>, Erro>> + Send;
}
