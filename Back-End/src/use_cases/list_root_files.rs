use std::sync::Arc;

use crate::domain::{Erro, StoredFile, UserId};
use crate::ports::FileRepoBd;

pub struct ListRootFiles<R> {
    pub repo: Arc<R>,
}

impl<R: FileRepoBd> ListRootFiles<R> {
    pub async fn execute(&self, owner: UserId) -> Result<Vec<StoredFile>, Erro> {
        self.repo.list_root(owner).await
    }
}
