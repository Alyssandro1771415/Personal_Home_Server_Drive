use std::future::Future;

use crate::domain::{Erro, UserId};

pub trait FileStore: Send + Sync {
    fn ensure_user_dir(&self, owner: UserId) -> impl Future<Output = Result<(), Erro>> + Send;
    fn put(
        &self,
        owner: UserId,
        key: &str,
        bytes: &[u8],
    ) -> impl Future<Output = Result<(), Erro>> + Send;
    fn get(
        &self,
        owner: UserId,
        key: &str,
    ) -> impl Future<Output = Result<Vec<u8>, Erro>> + Send;
}
