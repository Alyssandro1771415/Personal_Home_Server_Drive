use std::future::Future;

use chrono::{DateTime, Utc};

use crate::domain::{Erro, FileId, StoredFile, UserId};

pub trait FileRepoBd: Send + Sync {
    fn create(
        &self,
        owner: UserId,
        folder_id: Option<i64>,
        nome: String,
        mime: Option<String>,
        tamanho_bytes: i64,
        storage_key: String,
    ) -> impl Future<Output = Result<StoredFile, Erro>> + Send;

    fn list_in(
        &self,
        owner: UserId,
        folder_id: Option<i64>,
    ) -> impl Future<Output = Result<Vec<StoredFile>, Erro>> + Send;

    fn list_all_active(
        &self,
        owner: UserId,
    ) -> impl Future<Output = Result<Vec<StoredFile>, Erro>> + Send;

    fn get(&self, id: FileId) -> impl Future<Output = Result<Option<StoredFile>, Erro>> + Send;

    fn get_by_storage_key(
        &self,
        storage_key: &str,
    ) -> impl Future<Output = Result<Option<StoredFile>, Erro>> + Send;

    fn update_entry(
        &self,
        id: FileId,
        folder_id: Option<i64>,
        nome: String,
        storage_key: String,
        trashed_at: Option<DateTime<Utc>>,
    ) -> impl Future<Output = Result<(), Erro>> + Send;

    fn trash_in_folders(
        &self,
        owner: UserId,
        folder_ids: &[i32],
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<(), Erro>> + Send;

    fn restore_in_folders(
        &self,
        owner: UserId,
        folder_ids: &[i32],
        trashed_at: DateTime<Utc>,
    ) -> impl Future<Output = Result<(), Erro>> + Send;

    fn replace_key_prefix_in_folders(
        &self,
        owner: UserId,
        folder_ids: &[i32],
        old_prefix: &str,
        new_prefix: &str,
    ) -> impl Future<Output = Result<(), Erro>> + Send;

    fn list_trash(&self, owner: UserId) -> impl Future<Output = Result<Vec<StoredFile>, Erro>> + Send;

    fn list_expired(
        &self,
        before: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<StoredFile>, Erro>> + Send;

    fn list_in_folders(
        &self,
        folder_ids: &[i32],
    ) -> impl Future<Output = Result<Vec<StoredFile>, Erro>> + Send;

    fn delete(&self, id: FileId) -> impl Future<Output = Result<(), Erro>> + Send;

    fn delete_in_folders(
        &self,
        folder_ids: &[i32],
    ) -> impl Future<Output = Result<(), Erro>> + Send;
}
