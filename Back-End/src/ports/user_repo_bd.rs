use crate::domain::{Erro, User};

pub trait UserRepoBd: Send + Sync {
    fn list(&self) -> impl std::future::Future<Output = Result<Vec<User>, Erro>> + Send;
}
