use std::future::Future;

use chrono::{DateTime, Utc};

use crate::domain::{Erro, Folder, FolderId, UserId};

pub trait FolderRepoBd: Send + Sync {
    fn create(
        &self,
        owner: UserId,
        parent_id: Option<FolderId>,
        nome: String,
    ) -> impl Future<Output = Result<Folder, Erro>> + Send;

    fn list(
        &self,
        owner: UserId,
        parent_id: Option<FolderId>,
    ) -> impl Future<Output = Result<Vec<Folder>, Erro>> + Send;

    fn list_all_active(
        &self,
        owner: UserId,
    ) -> impl Future<Output = Result<Vec<Folder>, Erro>> + Send;

    fn get(&self, id: FolderId) -> impl Future<Output = Result<Option<Folder>, Erro>> + Send;

    fn rename(&self, id: FolderId, nome: String) -> impl Future<Output = Result<(), Erro>> + Send;

    fn set_parent(
        &self,
        id: FolderId,
        parent_id: Option<FolderId>,
    ) -> impl Future<Output = Result<(), Erro>> + Send;

    fn descendant_ids(
        &self,
        owner: UserId,
        id: FolderId,
    ) -> impl Future<Output = Result<Vec<i32>, Erro>> + Send;

    fn descendant_folders(
        &self,
        owner: UserId,
        id: FolderId,
    ) -> impl Future<Output = Result<Vec<Folder>, Erro>> + Send;

    fn trash_ids(
        &self,
        owner: UserId,
        ids: &[i32],
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<(), Erro>> + Send;

    fn restore_ids(
        &self,
        owner: UserId,
        ids: &[i32],
        trashed_at: DateTime<Utc>,
    ) -> impl Future<Output = Result<(), Erro>> + Send;

    fn list_trash(&self, owner: UserId) -> impl Future<Output = Result<Vec<Folder>, Erro>> + Send;

    fn list_expired(
        &self,
        before: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Folder>, Erro>> + Send;

    fn delete_ids(&self, ids: &[i32]) -> impl Future<Output = Result<(), Erro>> + Send;
}
