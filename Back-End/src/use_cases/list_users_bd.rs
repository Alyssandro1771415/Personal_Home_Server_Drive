use std::sync::Arc;

use crate::domain::{Erro, User};
use crate::ports::UserRepoBd;

pub struct ListUsers<R> {
    pub repo: Arc<R>,
}

impl<R: UserRepoBd> ListUsers<R> {
    pub async fn execute(&self) -> Result<Vec<User>, Erro> {
        self.repo.list().await
    }
}
