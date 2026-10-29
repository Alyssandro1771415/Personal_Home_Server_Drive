use std::sync::Arc;

use crate::domain::{validate_name, Erro, Folder, FolderId, UserId};
use crate::ports::{FileStore, FolderRepoBd, UserRepoBd};
use crate::use_cases::storage_paths::{folder_rel_dir, user_root};

const NOME_MAX: usize = 80;

pub struct CreateFolder<U, R, S> {
    pub users: Arc<U>,
    pub repo: Arc<R>,
    pub store: Arc<S>,
}

impl<U: UserRepoBd, R: FolderRepoBd, S: FileStore> CreateFolder<U, R, S> {
    pub async fn execute(
        &self,
        owner: UserId,
        parent_id: Option<FolderId>,
        nome: String,
    ) -> Result<Folder, Erro> {
        let nome = validate_name(&nome, NOME_MAX)?;
        let root = user_root(&*self.users, owner).await?;
        let parent_dir = folder_rel_dir(&*self.repo, owner, &root, parent_id).await?;

        let existing_siblings = self.repo.list(owner, parent_id).await?;
        if existing_siblings
            .iter()
            .any(|f| f.nome.eq_ignore_ascii_case(&nome))
        {
            return Err(Erro::Conflito);
        }

        let target_dir = format!("{parent_dir}/{nome}");
        if self.store.exists(&target_dir).await? {
            return Err(Erro::Conflito);
        }

        self.store.ensure_dir(&target_dir).await?;
        self.repo.create(owner, parent_id, nome).await
    }
}
