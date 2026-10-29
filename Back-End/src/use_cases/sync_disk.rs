use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::domain::{guess_mime, validate_name, Erro, FolderId, User};
use crate::ports::{FileRepoBd, FolderRepoBd, UserRepoBd};

const FOLDER_MAX: usize = 80;
const FILE_MAX: usize = 200;

#[derive(Default, Debug, Clone, Copy)]
pub struct SyncStats {
    pub folders_imported: usize,
    pub files_imported: usize,
}

/// Importa de forma aditiva pastas e arquivos presentes no disco (`<storage_root>/<user.name>`)
/// que ainda não estejam registrados no PostgreSQL (útil após rebuild do banco ou cópia via SSH/Samba).
pub async fn sync_user_from_disk<D: FolderRepoBd, F: FileRepoBd>(
    storage_root: &Path,
    user: &User,
    folders: &D,
    files: &F,
) -> Result<SyncStats, Erro> {
    let user_dir = storage_root.join(&user.name);
    tokio::fs::create_dir_all(&user_dir)
        .await
        .map_err(|_| Erro::Interno)?;

    let mut stats = SyncStats::default();
    let mut stack: Vec<(Option<FolderId>, PathBuf, String)> =
        vec![(None, user_dir, user.name.clone())];

    while let Some((parent_id, abs_dir, rel_dir)) = stack.pop() {
        let existing = folders.list(user.id, parent_id).await?;
        let mut by_name: HashMap<String, FolderId> = existing
            .into_iter()
            .map(|f| (f.nome, f.id))
            .collect();

        let mut rd = match tokio::fs::read_dir(&abs_dir).await {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        while let Ok(Some(entry)) = rd.next_entry().await {
            let Ok(ft) = entry.file_type().await else {
                continue;
            };
            if ft.is_symlink() {
                continue;
            }
            let os_name = entry.file_name();
            let Some(raw_name) = os_name.to_str() else {
                continue;
            };
            if raw_name.starts_with('.') {
                continue;
            }

            if ft.is_dir() {
                let Ok(clean_name) = validate_name(raw_name, FOLDER_MAX) else {
                    continue;
                };
                if clean_name != raw_name {
                    continue;
                }
                let child_rel = format!("{rel_dir}/{clean_name}");
                let folder_id = match by_name.get(&clean_name).copied() {
                    Some(id) => id,
                    None => {
                        let created = folders
                            .create(user.id, parent_id, clean_name.clone())
                            .await?;
                        by_name.insert(clean_name, created.id);
                        stats.folders_imported += 1;
                        created.id
                    }
                };
                stack.push((Some(folder_id), entry.path(), child_rel));
            } else if ft.is_file() {
                let Ok(clean_name) = validate_name(raw_name, FILE_MAX) else {
                    continue;
                };
                if clean_name != raw_name {
                    continue;
                }
                let child_rel = format!("{rel_dir}/{clean_name}");
                if files.get_by_storage_key(&child_rel).await?.is_none() {
                    let Ok(meta) = entry.metadata().await else {
                        continue;
                    };
                    let mime = guess_mime(&clean_name);
                    files
                        .create(
                            user.id,
                            parent_id.map(|id| id.0),
                            clean_name,
                            mime,
                            meta.len() as i64,
                            child_rel,
                        )
                        .await?;
                    stats.files_imported += 1;
                }
            }
        }
    }

    Ok(stats)
}

pub async fn sync_all_users<U: UserRepoBd, D: FolderRepoBd, F: FileRepoBd>(
    storage_root: &Path,
    users: &U,
    folders: &D,
    files: &F,
) -> Result<SyncStats, Erro> {
    let mut total = SyncStats::default();
    for user in users.list().await? {
        let stats = sync_user_from_disk(storage_root, &user, folders, files).await?;
        total.folders_imported += stats.folders_imported;
        total.files_imported += stats.files_imported;
    }
    Ok(total)
}
