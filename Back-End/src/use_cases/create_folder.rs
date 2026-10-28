use std::sync::Arc;

use crate::domain::{Erro, Folder, FolderId, UserId};
use crate::ports::FolderRepoBd;

const NOME_MAX: usize = 80;

pub struct CreateFolder<R> {
    pub repo: Arc<R>,
}

impl<R: FolderRepoBd> CreateFolder<R> {
    pub async fn execute(
        &self,
        owner: UserId,
        parent_id: Option<FolderId>,
        nome: String,
    ) -> Result<Folder, Erro> {
        let nome = nome.trim().to_string();
        if nome.is_empty() || nome.len() > NOME_MAX || nome.contains('/') {
            return Err(Erro::Validacao);
        }
        if let Some(parent) = parent_id {
            let folder = self.repo.get(parent).await?.ok_or(Erro::NaoEncontrado)?;
            if folder.owner_id != owner {
                return Err(Erro::NaoEncontrado);
            }
        }
        self.repo.create(owner, parent_id, nome).await
    }
}
