//! Arquivos cifrados no disco do servidor.

use std::path::{Path, PathBuf};

use crate::domain::{Erro, UserId};
use crate::ports::FileStore;

#[derive(Clone)]
pub struct FsStore {
    root: PathBuf,
}

impl FsStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn user_dir(&self, owner: UserId) -> PathBuf {
        self.root.join(owner.0.to_string())
    }

    fn blob_path(&self, owner: UserId, key: &str) -> Result<PathBuf, Erro> {
        uuid::Uuid::parse_str(key).map_err(|_| Erro::Validacao)?;
        Ok(self.user_dir(owner).join(key))
    }
}

impl FileStore for FsStore {
    async fn ensure_user_dir(&self, owner: UserId) -> Result<(), Erro> {
        tokio::fs::create_dir_all(self.user_dir(owner))
            .await
            .map_err(|e| {
                eprintln!("fs: {e}");
                Erro::Interno
            })
    }

    async fn put(&self, owner: UserId, key: &str, bytes: &[u8]) -> Result<(), Erro> {
        let path = self.blob_path(owner, key)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                eprintln!("fs: {e}");
                Erro::Interno
            })?;
        }
        tokio::fs::write(&path, bytes).await.map_err(|e| {
            eprintln!("fs: {e}");
            Erro::Interno
        })
    }

    async fn get(&self, owner: UserId, key: &str) -> Result<Vec<u8>, Erro> {
        let path = self.blob_path(owner, key)?;
        match tokio::fs::read(&path).await {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Erro::NaoEncontrado),
            Err(e) => {
                eprintln!("fs: {e}");
                Err(Erro::Interno)
            }
        }
    }

    async fn remove(&self, owner: UserId, key: &str) -> Result<(), Erro> {
        let path = self.blob_path(owner, key)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => {
                eprintln!("fs: {e}");
                Err(Erro::Interno)
            }
        }
    }
}

impl FsStore {
    pub fn root(&self) -> &Path {
        &self.root
    }
}
