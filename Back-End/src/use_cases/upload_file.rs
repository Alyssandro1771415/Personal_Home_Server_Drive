use std::sync::Arc;

use crate::domain::{validate_name, Erro, FolderId, StoredFile, UserId};
use crate::ports::{FileRepoBd, FileStore, FolderRepoBd, UserRepoBd};
use crate::use_cases::storage_paths::{folder_rel_dir, pick_free_name, user_root};

const NOME_MAX: usize = 200;

pub struct UploadFile<U, R, D, S> {
    pub users: Arc<U>,
    pub repo: Arc<R>,
    pub folders: Arc<D>,
    pub store: Arc<S>,
}

impl<U: UserRepoBd, R: FileRepoBd, D: FolderRepoBd, S: FileStore> UploadFile<U, R, D, S> {
    pub async fn execute(
        &self,
        owner: UserId,
        folder_id: Option<FolderId>,
        nome: String,
        mime: Option<String>,
        plaintext: Vec<u8>,
    ) -> Result<StoredFile, Erro> {
        // Remove qualquer prefixo de diretório caso o navegador envie caminho relativo no nome
        let base = nome.rsplit(['/', '\\']).next().unwrap_or(&nome);
        let clean_nome = validate_name(base, NOME_MAX)?;

        let root = user_root(&*self.users, owner).await?;
        let parent_dir = folder_rel_dir(&*self.folders, owner, &root, folder_id).await?;

        let (final_nome, storage_key) =
            pick_free_name(&*self.store, &parent_dir, &clean_nome, NOME_MAX).await?;

        let size = plaintext.len() as i64;
        self.store.put(&storage_key, &plaintext).await?;

        match self
            .repo
            .create(
                owner,
                folder_id.map(|id| id.0),
                final_nome,
                mime,
                size,
                storage_key.clone(),
            )
            .await
        {
            Ok(file) => Ok(file),
            Err(err) => {
                let _ = self.store.remove_file(&storage_key).await;
                Err(err)
            }
        }
    }
}
