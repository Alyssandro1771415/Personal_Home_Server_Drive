use std::future::Future;
use std::path::Path;

use crate::domain::{DiskStats, Erro};

pub trait FileStore: Send + Sync {
    fn ensure_dir(&self, rel_dir: &str) -> impl Future<Output = Result<(), Erro>> + Send;
    fn exists(&self, rel_path: &str) -> impl Future<Output = Result<bool, Erro>> + Send;
    fn is_dir(&self, rel_path: &str) -> impl Future<Output = Result<bool, Erro>> + Send;
    fn put(&self, rel_path: &str, bytes: &[u8]) -> impl Future<Output = Result<(), Erro>> + Send;
    fn put_temp(
        &self,
        temp_path: &Path,
        rel_path: &str,
    ) -> impl Future<Output = Result<(), Erro>> + Send;
    fn get(&self, rel_path: &str) -> impl Future<Output = Result<Vec<u8>, Erro>> + Send;
    fn move_file(
        &self,
        from_rel: &str,
        to_rel: &str,
    ) -> impl Future<Output = Result<(), Erro>> + Send;
    fn move_dir(
        &self,
        from_rel: &str,
        to_rel: &str,
    ) -> impl Future<Output = Result<(), Erro>> + Send;
    fn remove_file(&self, rel_path: &str) -> impl Future<Output = Result<(), Erro>> + Send;
    fn remove_dir_all(&self, rel_dir: &str) -> impl Future<Output = Result<(), Erro>> + Send;
    fn disk_usage(&self) -> impl Future<Output = Result<DiskStats, Erro>> + Send;
}
