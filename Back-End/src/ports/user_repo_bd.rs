use std::future::Future;
use crate::domain::{Erro, User, UserId, UserSecret, WrappedDek};

pub trait UserRepoBd: Send + Sync {
    fn list(&self) -> impl std::future::Future<Output = Result<Vec<User>, Erro>> + Send;
    fn get(&self, id: UserId) -> impl Future<Output = Result<Option<User>, Erro>> + Send;
    fn get_hashs(
        &self,
        nome: &str,
    ) -> impl Future<Output = Result<Option<UserSecret>, Erro>> + Send;
    fn create(
        &self,
        nome: String,
        password_hash: String,
        wrapped: WrappedDek,
    ) -> impl Future<Output = Result<UserId, Erro>> + Send;
}
