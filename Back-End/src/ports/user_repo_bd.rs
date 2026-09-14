use std::future::Future;
use crate::domain::{Erro, User, UserId, UserSecret};

pub trait UserRepoBd: Send + Sync {
    fn list(&self) -> impl std::future::Future<Output = Result<Vec<User>, Erro>> + Send;
    fn get_hashs(&self, id: UserId) -> impl Future<Output = Result<Option<UserSecret>, Erro>> + Send;
    fn create(
        &self,
        nome: String,
        password_hash: String,
    ) -> impl Future<Output = Result<UserId, Erro>> + Send;
}
