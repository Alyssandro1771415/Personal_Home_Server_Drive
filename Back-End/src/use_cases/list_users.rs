use std::sync::Arc;

use crate::domain::{Erro, User};
use crate::ports::UserRepo;

pub struct ListUsers<R> {
    pub repo: Arc<R>,
}

impl<R: UserRepo> ListUsers<R> {
    pub fn execute(&self) -> Result<Vec<User>, Erro> {
        self.repo.list()
    }
}
