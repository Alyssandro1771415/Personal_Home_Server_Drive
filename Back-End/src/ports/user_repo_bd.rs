use std::future::Future;

use crate::domain::{Erro, User, UserId, UserSecret};

pub trait UserRepoBd: Send + Sync {
    fn list(&self) -> impl Future<Output = Result<Vec<User>, Erro>> + Send;
    fn get(&self, id: UserId) -> impl Future<Output = Result<Option<User>, Erro>> + Send;
    fn get_by_name(&self, nome: &str) -> impl Future<Output = Result<Option<User>, Erro>> + Send;
    fn get_hashs(
        &self,
        nome: &str,
    ) -> impl Future<Output = Result<Option<UserSecret>, Erro>> + Send;
    fn create(
        &self,
        nome: String,
        password_hash: String,
    ) -> impl Future<Output = Result<UserId, Erro>> + Send;
    fn update_password(
        &self,
        id: UserId,
        password_hash: String,
    ) -> impl Future<Output = Result<(), Erro>> + Send;
}
