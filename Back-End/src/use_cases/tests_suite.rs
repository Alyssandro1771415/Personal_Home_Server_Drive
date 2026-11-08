use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

use crate::adapters::fs::FsStore;
use crate::domain::{
    Erro, FileId, Folder, FolderId, StoredFile, UploadConflictMode, User, UserId, UserSecret,
};
use crate::ports::{FileRepoBd, FileStore, FolderRepoBd, UserRepoBd};
use crate::use_cases::create_folder::CreateFolder;
use crate::use_cases::empty_trash::EmptyTrash;
use crate::use_cases::mutate_file::MutateFile;
use crate::use_cases::mutate_folder::MutateFolder;
use crate::use_cases::upload_file::{UploadFile, UploadPayload};

#[derive(Default)]
struct MemUsers {
    users: Mutex<Vec<User>>,
}

impl MemUsers {
    fn with_user(id: i64, name: &str) -> Self {
        Self {
            users: Mutex::new(vec![User {
                id: UserId(id),
                name: name.to_string(),
            }]),
        }
    }
}

impl UserRepoBd for MemUsers {
    async fn list(&self) -> Result<Vec<User>, Erro> {
        Ok(self.users.lock().unwrap().clone())
    }
    async fn get(&self, id: UserId) -> Result<Option<User>, Erro> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.id == id)
            .cloned())
    }
    async fn get_by_name(&self, nome: &str) -> Result<Option<User>, Erro> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.name == nome)
            .cloned())
    }
    async fn get_hashs(&self, _nome: &str) -> Result<Option<UserSecret>, Erro> {
        Ok(None)
    }
    async fn create(&self, _nome: String, _password_hash: String) -> Result<UserId, Erro> {
        Err(Erro::Interno)
    }
    async fn update_password(&self, _id: UserId, _password_hash: String) -> Result<(), Erro> {
        Ok(())
    }
}

#[derive(Default)]
struct MemFolders {
    next_id: Mutex<i64>,
    rows: Mutex<Vec<Folder>>,
}

impl FolderRepoBd for MemFolders {
    async fn create(
        &self,
        owner: UserId,
        parent_id: Option<FolderId>,
        nome: String,
    ) -> Result<Folder, Erro> {
        let mut nid = self.next_id.lock().unwrap();
        *nid += 1;
        let folder = Folder {
            id: FolderId(*nid),
            owner_id: owner,
            parent_id,
            nome,
            trashed_at: None,
        };
        self.rows.lock().unwrap().push(folder.clone());
        Ok(folder)
    }

    async fn list(&self, owner: UserId, parent_id: Option<FolderId>) -> Result<Vec<Folder>, Erro> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .filter(|f| f.owner_id == owner && f.parent_id == parent_id && f.trashed_at.is_none())
            .cloned()
            .collect())
    }

    async fn list_all_active(&self, owner: UserId) -> Result<Vec<Folder>, Erro> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .filter(|f| f.owner_id == owner && f.trashed_at.is_none())
            .cloned()
            .collect())
    }

    async fn get(&self, id: FolderId) -> Result<Option<Folder>, Erro> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|f| f.id == id)
            .cloned())
    }

    async fn rename(&self, id: FolderId, nome: String) -> Result<(), Erro> {
        for f in self.rows.lock().unwrap().iter_mut() {
            if f.id == id {
                f.nome = nome;
                break;
            }
        }
        Ok(())
    }

    async fn set_parent(&self, id: FolderId, parent_id: Option<FolderId>) -> Result<(), Erro> {
        for f in self.rows.lock().unwrap().iter_mut() {
            if f.id == id {
                f.parent_id = parent_id;
                break;
            }
        }
        Ok(())
    }

    async fn descendant_ids(&self, owner: UserId, id: FolderId) -> Result<Vec<i32>, Erro> {
        Ok(self
            .descendant_folders(owner, id)
            .await?
            .into_iter()
            .map(|f| f.id.0 as i32)
            .collect())
    }

    async fn descendant_folders(&self, owner: UserId, id: FolderId) -> Result<Vec<Folder>, Erro> {
        let rows = self.rows.lock().unwrap().clone();
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            for f in &rows {
                if f.owner_id == owner && f.id == cur {
                    out.push(f.clone());
                } else if f.owner_id == owner && f.parent_id == Some(cur) {
                    stack.push(f.id);
                }
            }
        }
        Ok(out)
    }

    async fn trash_ids(
        &self,
        owner: UserId,
        ids: &[i32],
        at: DateTime<Utc>,
    ) -> Result<(), Erro> {
        for f in self.rows.lock().unwrap().iter_mut() {
            if f.owner_id == owner && ids.contains(&(f.id.0 as i32)) && f.trashed_at.is_none() {
                f.trashed_at = Some(at);
            }
        }
        Ok(())
    }

    async fn restore_ids(
        &self,
        owner: UserId,
        ids: &[i32],
        trashed_at: DateTime<Utc>,
    ) -> Result<(), Erro> {
        for f in self.rows.lock().unwrap().iter_mut() {
            if f.owner_id == owner && ids.contains(&(f.id.0 as i32)) && f.trashed_at == Some(trashed_at)
            {
                f.trashed_at = None;
            }
        }
        Ok(())
    }

    async fn list_trash(&self, owner: UserId) -> Result<Vec<Folder>, Erro> {
        let rows = self.rows.lock().unwrap().clone();
        Ok(rows
            .iter()
            .filter(|f| {
                f.owner_id == owner
                    && f.trashed_at.is_some()
                    && match f.parent_id {
                        None => true,
                        Some(pid) => !rows
                            .iter()
                            .any(|p| p.id == pid && p.trashed_at.is_some()),
                    }
            })
            .cloned()
            .collect())
    }

    async fn list_expired(&self, _before: DateTime<Utc>) -> Result<Vec<Folder>, Erro> {
        Ok(Vec::new())
    }

    async fn delete_ids(&self, ids: &[i32]) -> Result<(), Erro> {
        self.rows
            .lock()
            .unwrap()
            .retain(|f| !ids.contains(&(f.id.0 as i32)));
        Ok(())
    }
}

#[derive(Default)]
struct MemFiles {
    next_id: Mutex<i64>,
    rows: Mutex<Vec<StoredFile>>,
}

impl FileRepoBd for MemFiles {
    async fn create(
        &self,
        owner: UserId,
        folder_id: Option<i64>,
        nome: String,
        mime: Option<String>,
        tamanho_bytes: i64,
        storage_key: String,
    ) -> Result<StoredFile, Erro> {
        let mut rows = self.rows.lock().unwrap();
        if rows.iter().any(|r| r.storage_key == storage_key) {
            return Err(Erro::Conflito);
        }
        let mut nid = self.next_id.lock().unwrap();
        *nid += 1;
        let file = StoredFile {
            id: FileId(*nid),
            owner_id: owner,
            folder_id,
            nome,
            mime,
            tamanho_bytes,
            storage_key,
            trashed_at: None,
        };
        rows.push(file.clone());
        Ok(file)
    }

    async fn list_in(
        &self,
        owner: UserId,
        folder_id: Option<i64>,
    ) -> Result<Vec<StoredFile>, Erro> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .filter(|f| f.owner_id == owner && f.folder_id == folder_id && f.trashed_at.is_none())
            .cloned()
            .collect())
    }

    async fn list_all_active(&self, owner: UserId) -> Result<Vec<StoredFile>, Erro> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .filter(|f| f.owner_id == owner && f.trashed_at.is_none())
            .cloned()
            .collect())
    }

    async fn get(&self, id: FileId) -> Result<Option<StoredFile>, Erro> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|f| f.id == id)
            .cloned())
    }

    async fn get_by_storage_key(&self, storage_key: &str) -> Result<Option<StoredFile>, Erro> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|f| f.storage_key == storage_key)
            .cloned())
    }

    async fn update_entry(
        &self,
        id: FileId,
        folder_id: Option<i64>,
        nome: String,
        storage_key: String,
        trashed_at: Option<DateTime<Utc>>,
    ) -> Result<(), Erro> {
        for f in self.rows.lock().unwrap().iter_mut() {
            if f.id == id {
                f.folder_id = folder_id;
                f.nome = nome;
                f.storage_key = storage_key;
                f.trashed_at = trashed_at;
                return Ok(());
            }
        }
        Err(Erro::NaoEncontrado)
    }

    async fn overwrite_content(
        &self,
        id: FileId,
        mime: Option<String>,
        tamanho_bytes: i64,
    ) -> Result<StoredFile, Erro> {
        for f in self.rows.lock().unwrap().iter_mut() {
            if f.id == id {
                f.mime = mime;
                f.tamanho_bytes = tamanho_bytes;
                f.trashed_at = None;
                return Ok(f.clone());
            }
        }
        Err(Erro::NaoEncontrado)
    }

    async fn trash_in_folders(
        &self,
        owner: UserId,
        folder_ids: &[i32],
        at: DateTime<Utc>,
    ) -> Result<(), Erro> {
        for f in self.rows.lock().unwrap().iter_mut() {
            if let Some(fid) = f.folder_id {
                if f.owner_id == owner
                    && folder_ids.contains(&(fid as i32))
                    && f.trashed_at.is_none()
                {
                    f.trashed_at = Some(at);
                }
            }
        }
        Ok(())
    }

    async fn restore_in_folders(
        &self,
        owner: UserId,
        folder_ids: &[i32],
        trashed_at: DateTime<Utc>,
    ) -> Result<(), Erro> {
        for f in self.rows.lock().unwrap().iter_mut() {
            if let Some(fid) = f.folder_id {
                if f.owner_id == owner
                    && folder_ids.contains(&(fid as i32))
                    && f.trashed_at == Some(trashed_at)
                {
                    f.trashed_at = None;
                }
            }
        }
        Ok(())
    }

    async fn replace_key_prefix_in_folders(
        &self,
        owner: UserId,
        folder_ids: &[i32],
        old_prefix: &str,
        new_prefix: &str,
    ) -> Result<(), Erro> {
        for f in self.rows.lock().unwrap().iter_mut() {
            if let Some(fid) = f.folder_id {
                if f.owner_id == owner
                    && folder_ids.contains(&(fid as i32))
                    && f.storage_key.starts_with(old_prefix)
                {
                    let suffix = &f.storage_key[old_prefix.len()..];
                    f.storage_key = format!("{new_prefix}{suffix}");
                }
            }
        }
        Ok(())
    }

    async fn list_trash(&self, owner: UserId) -> Result<Vec<StoredFile>, Erro> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .filter(|f| f.owner_id == owner && f.trashed_at.is_some())
            .cloned()
            .collect())
    }

    async fn list_expired(&self, _before: DateTime<Utc>) -> Result<Vec<StoredFile>, Erro> {
        Ok(Vec::new())
    }

    async fn list_in_folders(&self, folder_ids: &[i32]) -> Result<Vec<StoredFile>, Erro> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .filter(|f| {
                f.folder_id
                    .map(|fid| folder_ids.contains(&(fid as i32)))
                    .unwrap_or(false)
            })
            .cloned()
            .collect())
    }

    async fn delete(&self, id: FileId) -> Result<(), Erro> {
        self.rows.lock().unwrap().retain(|f| f.id != id);
        Ok(())
    }

    async fn delete_in_folders(&self, folder_ids: &[i32]) -> Result<(), Erro> {
        self.rows.lock().unwrap().retain(|f| {
            !f.folder_id
                .map(|fid| folder_ids.contains(&(fid as i32)))
                .unwrap_or(false)
        });
        Ok(())
    }
}

#[tokio::test]
async fn upload_conflict_modes_reject_keep_both_and_overwrite() {
    let tmp = std::env::temp_dir().join(format!("drive_uc_upload_{}", uuid::Uuid::new_v4()));
    let users = Arc::new(MemUsers::with_user(1, "alyssandro"));
    let folders = Arc::new(MemFolders::default());
    let files = Arc::new(MemFiles::default());
    let store = Arc::new(FsStore::new(&tmp));
    store.ensure_dir("alyssandro").await.unwrap();

    let uc = UploadFile {
        users: Arc::clone(&users),
        repo: Arc::clone(&files),
        folders: Arc::clone(&folders),
        store: Arc::clone(&store),
    };

    // Primeiro envio
    let first = uc
        .execute(
            UserId(1),
            None,
            "foto.jpg".to_string(),
            Some("image/jpeg".to_string()),
            UploadPayload::Bytes(b"antigo".to_vec()),
            UploadConflictMode::Reject,
        )
        .await
        .unwrap();
    assert_eq!(first.nome, "foto.jpg");
    assert_eq!(first.tamanho_bytes, 6);

    // Reject detecta conflito sem alterar o arquivo original
    let conflict = uc
        .execute(
            UserId(1),
            None,
            "foto.jpg".to_string(),
            Some("image/jpeg".to_string()),
            UploadPayload::Bytes(b"tentativa".to_vec()),
            UploadConflictMode::Reject,
        )
        .await;
    assert!(matches!(conflict.unwrap_err(), Erro::Conflito));
    assert_eq!(store.get("alyssandro/foto.jpg").await.unwrap(), b"antigo");

    // KeepBoth cria um segundo arquivo mantendo o antigo
    let second = uc
        .execute(
            UserId(1),
            None,
            "foto.jpg".to_string(),
            Some("image/jpeg".to_string()),
            UploadPayload::Bytes(b"segundo".to_vec()),
            UploadConflictMode::KeepBoth,
        )
        .await
        .unwrap();
    assert_eq!(second.nome, "foto (1).jpg");
    assert_ne!(second.id, first.id);

    // Overwrite substitui o conteúdo em disco e atualiza metadados do mesmo ID
    let overwritten = uc
        .execute(
            UserId(1),
            None,
            "foto.jpg".to_string(),
            Some("image/jpeg".to_string()),
            UploadPayload::Bytes(b"conteudo novo maior".to_vec()),
            UploadConflictMode::Overwrite,
        )
        .await
        .unwrap();
    assert_eq!(overwritten.id, first.id);
    assert_eq!(overwritten.tamanho_bytes, b"conteudo novo maior".len() as i64);
    assert_eq!(
        store.get("alyssandro/foto.jpg").await.unwrap(),
        b"conteudo novo maior"
    );

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn batch_move_and_empty_trash_work_end_to_end() {
    let tmp = std::env::temp_dir().join(format!("drive_uc_batch_{}", uuid::Uuid::new_v4()));
    let users = Arc::new(MemUsers::with_user(1, "alyssandro"));
    let folders = Arc::new(MemFolders::default());
    let files = Arc::new(MemFiles::default());
    let store = Arc::new(FsStore::new(&tmp));
    store.ensure_dir("alyssandro").await.unwrap();

    let create_folder = CreateFolder {
        users: Arc::clone(&users),
        repo: Arc::clone(&folders),
        store: Arc::clone(&store),
    };
    let dest = create_folder
        .execute(UserId(1), None, "Destino".to_string())
        .await
        .unwrap();
    let old_folder = create_folder
        .execute(UserId(1), None, "Temporaria".to_string())
        .await
        .unwrap();

    let upload = UploadFile {
        users: Arc::clone(&users),
        repo: Arc::clone(&files),
        folders: Arc::clone(&folders),
        store: Arc::clone(&store),
    };

    let f1 = upload
        .execute(
            UserId(1),
            None,
            "a.txt".to_string(),
            None,
            UploadPayload::Bytes(b"111".to_vec()),
            UploadConflictMode::Reject,
        )
        .await
        .unwrap();
    let f2 = upload
        .execute(
            UserId(1),
            None,
            "b.txt".to_string(),
            None,
            UploadPayload::Bytes(b"222".to_vec()),
            UploadConflictMode::Reject,
        )
        .await
        .unwrap();
    // Cria um arquivo com mesmo nome 'b.txt' já dentro de Destino para testar conflito parcial
    let _existing_in_dest = upload
        .execute(
            UserId(1),
            Some(dest.id),
            "b.txt".to_string(),
            None,
            UploadPayload::Bytes(b"existente".to_vec()),
            UploadConflictMode::Reject,
        )
        .await
        .unwrap();
    // Arquivo dentro de Temporaria (que irá para a lixeira junto com a pasta)
    let _nested = upload
        .execute(
            UserId(1),
            Some(old_folder.id),
            "interno.txt".to_string(),
            None,
            UploadPayload::Bytes(b"lixo".to_vec()),
            UploadConflictMode::Reject,
        )
        .await
        .unwrap();

    let mutate_file = MutateFile {
        users: Arc::clone(&users),
        files: Arc::clone(&files),
        folders: Arc::clone(&folders),
        store: Arc::clone(&store),
    };

    // Move f1 e f2 juntos para Destino: f1 move, f2 acusa conflito sem falhar o lote inteiro
    let batch_res = mutate_file
        .batch_move_to(UserId(1), &[f1.id, f2.id], Some(dest.id))
        .await
        .unwrap();
    assert_eq!(batch_res.moved_ids, vec![f1.id.0]);
    assert_eq!(batch_res.conflict_ids, vec![f2.id.0]);
    assert_eq!(batch_res.conflict_names, vec!["b.txt".to_string()]);
    assert!(store.exists("alyssandro/Destino/a.txt").await.unwrap());
    assert!(store.exists("alyssandro/b.txt").await.unwrap());

    // Envia f2 e a pasta Temporaria para a lixeira e executa EmptyTrash
    mutate_file.trash(UserId(1), f2.id).await.unwrap();
    let mutate_folder = MutateFolder {
        users: Arc::clone(&users),
        folders: Arc::clone(&folders),
        files: Arc::clone(&files),
        store: Arc::clone(&store),
    };
    mutate_folder.trash(UserId(1), old_folder.id).await.unwrap();

    assert_eq!(folders.list_trash(UserId(1)).await.unwrap().len(), 1);
    assert!(!files.list_trash(UserId(1)).await.unwrap().is_empty());

    let empty_trash = EmptyTrash {
        users: Arc::clone(&users),
        files: Arc::clone(&files),
        folders: Arc::clone(&folders),
        store: Arc::clone(&store),
    };
    empty_trash.execute(UserId(1)).await.unwrap();

    assert!(folders.list_trash(UserId(1)).await.unwrap().is_empty());
    assert!(files.list_trash(UserId(1)).await.unwrap().is_empty());
    assert!(!store.exists("alyssandro/.lixeira").await.unwrap());
    // Arquivos ativos em Destino permanecem intactos
    assert!(store.exists("alyssandro/Destino/a.txt").await.unwrap());

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}
