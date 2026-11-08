//! Armazenamento direto de pastas e arquivos reais no disco do servidor.

use std::path::{Component, Path, PathBuf};

use crate::domain::{DiskStats, Erro};
use crate::ports::FileStore;

const TEMP_UPLOAD_DIR: &str = ".tmp_uploads";

#[derive(Clone)]
pub struct FsStore {
    root: PathBuf,
}

impl FsStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve_rel(&self, rel: &str) -> Result<PathBuf, Erro> {
        if rel.is_empty() || rel.contains('\\') || rel.contains('\0') {
            return Err(Erro::Validacao);
        }
        if rel
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
        {
            return Err(Erro::Validacao);
        }

        let mut out = self.root.clone();
        let mut count = 0usize;
        for comp in Path::new(rel).components() {
            match comp {
                Component::Normal(seg) => {
                    out.push(seg);
                    count += 1;
                }
                _ => return Err(Erro::Validacao),
            }
        }
        if count == 0 {
            return Err(Erro::Validacao);
        }
        Ok(out)
    }

    pub async fn open_read(&self, rel_path: &str) -> Result<(tokio::fs::File, u64), Erro> {
        let path = self.resolve_rel(rel_path)?;
        let file = match tokio::fs::File::open(&path).await {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Erro::NaoEncontrado),
            Err(e) => {
                eprintln!("fs open_read: {e}");
                return Err(Erro::Interno);
            }
        };
        let meta = file.metadata().await.map_err(|e| {
            eprintln!("fs open_read metadata: {e}");
            Erro::Interno
        })?;
        if !meta.is_file() {
            return Err(Erro::NaoEncontrado);
        }
        Ok((file, meta.len()))
    }

    pub async fn create_temp_file(&self) -> Result<(PathBuf, tokio::fs::File), Erro> {
        let dir = self.root.join(TEMP_UPLOAD_DIR);
        tokio::fs::create_dir_all(&dir).await.map_err(|e| {
            eprintln!("fs create_temp_file dir: {e}");
            Erro::Interno
        })?;
        let path = dir.join(format!("upload_{}.part", uuid::Uuid::new_v4()));
        let file = tokio::fs::File::create(&path).await.map_err(|e| {
            eprintln!("fs create_temp_file create: {e}");
            Erro::Interno
        })?;
        Ok((path, file))
    }

    pub async fn cleanup_temp_uploads(&self) {
        let dir = self.root.join(TEMP_UPLOAD_DIR);
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}

#[cfg(unix)]
fn statvfs_disk_usage(path: &Path) -> Result<DiskStats, Erro> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| Erro::Interno)?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if rc != 0 {
        eprintln!("fs statvfs erro em {}: {}", path.display(), std::io::Error::last_os_error());
        return Err(Erro::Interno);
    }

    let block_size = if stat.f_frsize > 0 {
        stat.f_frsize as u64
    } else {
        stat.f_bsize as u64
    };
    let total_bytes = (stat.f_blocks as u64).saturating_mul(block_size);
    let free_bytes = (stat.f_bfree as u64).saturating_mul(block_size);
    let available_bytes = (stat.f_bavail as u64).saturating_mul(block_size);
    let used_bytes = total_bytes.saturating_sub(free_bytes);

    Ok(DiskStats {
        used_bytes,
        total_bytes,
        available_bytes,
    })
}

#[cfg(not(unix))]
fn statvfs_disk_usage(_path: &Path) -> Result<DiskStats, Erro> {
    Err(Erro::Interno)
}

impl FileStore for FsStore {
    async fn ensure_dir(&self, rel_dir: &str) -> Result<(), Erro> {
        let path = self.resolve_rel(rel_dir)?;
        tokio::fs::create_dir_all(&path).await.map_err(|e| {
            eprintln!("fs ensure_dir: {e}");
            Erro::Interno
        })
    }

    async fn exists(&self, rel_path: &str) -> Result<bool, Erro> {
        let path = self.resolve_rel(rel_path)?;
        tokio::fs::try_exists(&path).await.map_err(|e| {
            eprintln!("fs exists: {e}");
            Erro::Interno
        })
    }

    async fn is_dir(&self, rel_path: &str) -> Result<bool, Erro> {
        let path = self.resolve_rel(rel_path)?;
        match tokio::fs::metadata(&path).await {
            Ok(meta) => Ok(meta.is_dir()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => {
                eprintln!("fs is_dir: {e}");
                Err(Erro::Interno)
            }
        }
    }

    async fn put(&self, rel_path: &str, bytes: &[u8]) -> Result<(), Erro> {
        let path = self.resolve_rel(rel_path)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                eprintln!("fs put create_dir_all: {e}");
                Erro::Interno
            })?;
        }
        tokio::fs::write(&path, bytes).await.map_err(|e| {
            eprintln!("fs put write: {e}");
            Erro::Interno
        })
    }

    async fn put_temp(&self, temp_path: &Path, rel_path: &str) -> Result<(), Erro> {
        let path = self.resolve_rel(rel_path)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                eprintln!("fs put_temp create_dir_all: {e}");
                Erro::Interno
            })?;
        }
        match tokio::fs::rename(temp_path, &path).await {
            Ok(()) => Ok(()),
            Err(_) => {
                tokio::fs::copy(temp_path, &path).await.map_err(|e| {
                    eprintln!("fs put_temp copy fallback: {e}");
                    Erro::Interno
                })?;
                let _ = tokio::fs::remove_file(temp_path).await;
                Ok(())
            }
        }
    }

    async fn get(&self, rel_path: &str) -> Result<Vec<u8>, Erro> {
        let path = self.resolve_rel(rel_path)?;
        match tokio::fs::read(&path).await {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Erro::NaoEncontrado),
            Err(e) => {
                eprintln!("fs get: {e}");
                Err(Erro::Interno)
            }
        }
    }

    async fn move_file(&self, from_rel: &str, to_rel: &str) -> Result<(), Erro> {
        if from_rel == to_rel {
            return Ok(());
        }
        let from_path = self.resolve_rel(from_rel)?;
        let to_path = self.resolve_rel(to_rel)?;
        if let Some(parent) = to_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                eprintln!("fs move_file parent: {e}");
                Erro::Interno
            })?;
        }
        match tokio::fs::rename(&from_path, &to_path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Erro::NaoEncontrado),
            Err(e) => {
                eprintln!("fs move_file rename: {e}");
                Err(Erro::Interno)
            }
        }
    }

    async fn move_dir(&self, from_rel: &str, to_rel: &str) -> Result<(), Erro> {
        if from_rel == to_rel {
            return Ok(());
        }
        let from_path = self.resolve_rel(from_rel)?;
        let to_path = self.resolve_rel(to_rel)?;
        if let Some(parent) = to_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                eprintln!("fs move_dir parent: {e}");
                Erro::Interno
            })?;
        }
        match tokio::fs::rename(&from_path, &to_path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tokio::fs::create_dir_all(&to_path).await.map_err(|e2| {
                    eprintln!("fs move_dir fallback create: {e2}");
                    Erro::Interno
                })
            }
            Err(e) => {
                eprintln!("fs move_dir rename: {e}");
                Err(Erro::Interno)
            }
        }
    }

    async fn remove_file(&self, rel_path: &str) -> Result<(), Erro> {
        let path = self.resolve_rel(rel_path)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => {
                eprintln!("fs remove_file: {e}");
                Err(Erro::Interno)
            }
        }
    }

    async fn remove_dir_all(&self, rel_dir: &str) -> Result<(), Erro> {
        let path = self.resolve_rel(rel_dir)?;
        match tokio::fs::remove_dir_all(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => {
                eprintln!("fs remove_dir_all: {e}");
                Err(Erro::Interno)
            }
        }
    }

    async fn disk_usage(&self) -> Result<DiskStats, Erro> {
        tokio::fs::create_dir_all(&self.root).await.map_err(|e| {
            eprintln!("fs disk_usage ensure root: {e}");
            Erro::Interno
        })?;
        statvfs_disk_usage(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn real_tree_operations_and_path_traversal_guard() {
        let tmp = std::env::temp_dir().join(format!("drive_fs_test_{}", uuid::Uuid::new_v4()));
        let store = FsStore::new(&tmp);

        // Path traversal must always fail
        assert!(store.resolve_rel("../etc/passwd").is_err());
        assert!(store.resolve_rel("/etc/passwd").is_err());
        assert!(store.resolve_rel("user/../../secret").is_err());
        assert!(store.resolve_rel("user//file.txt").is_err());
        assert!(store.resolve_rel("").is_err());

        // Real folder & file hierarchy
        store.ensure_dir("alyssandro/Fotos/Viagem").await.unwrap();
        assert!(store.exists("alyssandro/Fotos/Viagem").await.unwrap());

        store
            .put("alyssandro/Fotos/Viagem/praia.txt", b"sol e mar")
            .await
            .unwrap();
        assert_eq!(
            store.get("alyssandro/Fotos/Viagem/praia.txt").await.unwrap(),
            b"sol e mar"
        );

        // Rename folder moves nested files on disk automatically
        store
            .move_dir("alyssandro/Fotos", "alyssandro/Minhas Fotos")
            .await
            .unwrap();
        assert!(!store.exists("alyssandro/Fotos").await.unwrap());
        assert_eq!(
            store
                .get("alyssandro/Minhas Fotos/Viagem/praia.txt")
                .await
                .unwrap(),
            b"sol e mar"
        );

        // Trash file moves into .lixeira
        store
            .move_file(
                "alyssandro/Minhas Fotos/Viagem/praia.txt",
                "alyssandro/.lixeira/file_1_praia.txt",
            )
            .await
            .unwrap();
        assert!(store
            .exists("alyssandro/.lixeira/file_1_praia.txt")
            .await
            .unwrap());

        store
            .remove_file("alyssandro/.lixeira/file_1_praia.txt")
            .await
            .unwrap();

        // Disk stats, is_dir, open_read & temp upload commit
        let stats = store.disk_usage().await.unwrap();
        assert!(stats.total_bytes > 0);
        assert!(stats.used_bytes <= stats.total_bytes);

        assert!(store.is_dir("alyssandro/Minhas Fotos").await.unwrap());
        assert!(!store.is_dir("alyssandro/inexistente").await.unwrap());

        let (tmp_file_path, mut tmp_file) = store.create_temp_file().await.unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut tmp_file, b"conteudo em pedacos")
            .await
            .unwrap();
        drop(tmp_file);
        store
            .put_temp(&tmp_file_path, "alyssandro/video.bin")
            .await
            .unwrap();
        let (_reader, len) = store.open_read("alyssandro/video.bin").await.unwrap();
        assert_eq!(len, b"conteudo em pedacos".len() as u64);

        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }
}
