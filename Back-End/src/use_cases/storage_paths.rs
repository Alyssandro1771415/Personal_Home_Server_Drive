use crate::domain::{
    dedupe_name, Erro, FileId, FolderId, UserId, TRASH_DIR_NAME,
};
use crate::ports::{FileStore, FolderRepoBd, UserRepoBd};

pub async fn user_root<U: UserRepoBd>(users: &U, owner: UserId) -> Result<String, Erro> {
    let user = users.get(owner).await?.ok_or(Erro::NaoEncontrado)?;
    Ok(user.name)
}

pub async fn folder_rel_dir<D: FolderRepoBd>(
    folders: &D,
    owner: UserId,
    root_name: &str,
    folder_id: Option<FolderId>,
) -> Result<String, Erro> {
    let Some(start) = folder_id else {
        return Ok(root_name.to_string());
    };

    let mut segments = Vec::new();
    let mut current = Some(start);
    let mut guard = 0usize;

    while let Some(id) = current {
        guard += 1;
        if guard > 32 {
            return Err(Erro::Interno);
        }
        let folder = folders.get(id).await?.ok_or(Erro::NaoEncontrado)?;
        if folder.owner_id != owner || folder.trashed_at.is_some() {
            return Err(Erro::NaoEncontrado);
        }
        segments.push(folder.nome);
        current = folder.parent_id;
    }

    segments.reverse();
    let mut rel = String::from(root_name);
    for seg in segments {
        rel.push('/');
        rel.push_str(&seg);
    }
    Ok(rel)
}

pub fn trash_file_key(root_name: &str, id: FileId, nome: &str) -> String {
    format!("{root_name}/{TRASH_DIR_NAME}/file_{}_{}", id.0, nome)
}

pub fn trash_folder_dir(root_name: &str, id: FolderId, nome: &str) -> String {
    format!("{root_name}/{TRASH_DIR_NAME}/folder_{}_{}", id.0, nome)
}

/// Escolhe `nome` ou `"nome (1)"`, `"nome (2)"`... caso o caminho já exista no disco.
pub async fn pick_free_name<S: FileStore>(
    store: &S,
    parent_dir: &str,
    desired_nome: &str,
    max_len: usize,
) -> Result<(String, String), Erro> {
    let initial_rel = format!("{parent_dir}/{desired_nome}");
    if !store.exists(&initial_rel).await? {
        return Ok((desired_nome.to_string(), initial_rel));
    }

    for attempt in 1..=9999 {
        let candidate = dedupe_name(desired_nome, attempt, max_len);
        let rel = format!("{parent_dir}/{candidate}");
        if !store.exists(&rel).await? {
            return Ok((candidate, rel));
        }
    }

    Err(Erro::Conflito)
}
