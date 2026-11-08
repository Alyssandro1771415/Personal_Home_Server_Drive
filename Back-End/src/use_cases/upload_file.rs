use std::path::PathBuf;
use std::sync::Arc;

use crate::domain::{
    guess_mime, validate_name, Erro, FolderId, StoredFile, UploadConflictMode, UserId,
};
use crate::ports::{FileRepoBd, FileStore, FolderRepoBd, UserRepoBd};
use crate::use_cases::storage_paths::{folder_rel_dir, pick_free_name, user_root};

const NOME_MAX: usize = 200;

pub enum UploadPayload {
    Bytes(Vec<u8>),
    TempFile { path: PathBuf, size: u64 },
}

impl UploadPayload {
    fn size_i64(&self) -> i64 {
        match self {
            Self::Bytes(b) => b.len() as i64,
            Self::TempFile { size, .. } => *size as i64,
        }
    }

    async fn persist<S: FileStore>(self, store: &S, rel_path: &str) -> Result<(), Erro> {
        match self {
            Self::Bytes(bytes) => store.put(rel_path, &bytes).await,
            Self::TempFile { path, .. } => {
                let res = store.put_temp(&path, rel_path).await;
                if res.is_err() {
                    let _ = tokio::fs::remove_file(&path).await;
                }
                res
            }
        }
    }

    async fn cleanup(self) {
        if let Self::TempFile { path, .. } = self {
            let _ = tokio::fs::remove_file(&path).await;
        }
    }
}

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
        payload: UploadPayload,
        conflict_mode: UploadConflictMode,
    ) -> Result<StoredFile, Erro> {
        // Remove qualquer prefixo de diretório caso o navegador envie caminho relativo no nome
        let base = nome.rsplit(['/', '\\']).next().unwrap_or(&nome);
        let clean_nome = match validate_name(base, NOME_MAX) {
            Ok(n) => n,
            Err(e) => {
                payload.cleanup().await;
                return Err(e);
            }
        };

        let root = match user_root(&*self.users, owner).await {
            Ok(r) => r,
            Err(e) => {
                payload.cleanup().await;
                return Err(e);
            }
        };
        let parent_dir = match folder_rel_dir(&*self.folders, owner, &root, folder_id).await {
            Ok(d) => d,
            Err(e) => {
                payload.cleanup().await;
                return Err(e);
            }
        };

        let resolved_mime = match mime.as_deref() {
            None | Some("") | Some("application/octet-stream") => {
                guess_mime(&clean_nome).or(mime)
            }
            _ => mime,
        };
        let size = payload.size_i64();

        match conflict_mode {
            UploadConflictMode::Overwrite => {
                let initial_rel = format!("{parent_dir}/{clean_nome}");
                match self.store.is_dir(&initial_rel).await {
                    Ok(true) => {
                        payload.cleanup().await;
                        return Err(Erro::Conflito);
                    }
                    Ok(false) => {}
                    Err(e) => {
                        payload.cleanup().await;
                        return Err(e);
                    }
                }

                let existing = match self.repo.get_by_storage_key(&initial_rel).await {
                    Ok(ex) => ex,
                    Err(e) => {
                        payload.cleanup().await;
                        return Err(e);
                    }
                };

                if let Some(ref ex) = existing {
                    if ex.owner_id != owner {
                        payload.cleanup().await;
                        return Err(Erro::Conflito);
                    }
                }

                payload.persist(&*self.store, &initial_rel).await?;

                if let Some(ex) = existing {
                    self.repo
                        .overwrite_content(ex.id, resolved_mime, size)
                        .await
                } else {
                    match self
                        .repo
                        .create(
                            owner,
                            folder_id.map(|id| id.0),
                            clean_nome,
                            resolved_mime,
                            size,
                            initial_rel.clone(),
                        )
                        .await
                    {
                        Ok(file) => Ok(file),
                        Err(err) => {
                            let _ = self.store.remove_file(&initial_rel).await;
                            Err(err)
                        }
                    }
                }
            }
            UploadConflictMode::Reject => {
                let initial_rel = format!("{parent_dir}/{clean_nome}");
                match self.store.exists(&initial_rel).await {
                    Ok(true) => {
                        payload.cleanup().await;
                        return Err(Erro::Conflito);
                    }
                    Ok(false) => {}
                    Err(e) => {
                        payload.cleanup().await;
                        return Err(e);
                    }
                }

                payload.persist(&*self.store, &initial_rel).await?;

                match self
                    .repo
                    .create(
                        owner,
                        folder_id.map(|id| id.0),
                        clean_nome,
                        resolved_mime,
                        size,
                        initial_rel.clone(),
                    )
                    .await
                {
                    Ok(file) => Ok(file),
                    Err(err) => {
                        let _ = self.store.remove_file(&initial_rel).await;
                        Err(err)
                    }
                }
            }
            UploadConflictMode::KeepBoth => {
                let (final_nome, storage_key) =
                    match pick_free_name(&*self.store, &parent_dir, &clean_nome, NOME_MAX).await {
                        Ok(res) => res,
                        Err(e) => {
                            payload.cleanup().await;
                            return Err(e);
                        }
                    };

                payload.persist(&*self.store, &storage_key).await?;

                match self
                    .repo
                    .create(
                        owner,
                        folder_id.map(|id| id.0),
                        final_nome,
                        resolved_mime,
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
    }
}
