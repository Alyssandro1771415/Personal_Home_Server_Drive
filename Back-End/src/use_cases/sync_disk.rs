use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::domain::{
    classify_file, guess_mime, resolve_timestamps, same_second, validate_name, video_duration_ms,
    Erro, FileCategory, FileId, Folder, FolderId, User,
};
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
        let mut by_name: HashMap<String, Folder> = existing
            .into_iter()
            .map(|f| (f.nome.clone(), f))
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
                let (folder_id, existing_created, existing_updated) = match by_name.get(&clean_name) {
                    Some(folder) => (folder.id, Some(folder.created_at), Some(folder.updated_at)),
                    None => {
                        let created = folders
                            .create(user.id, parent_id, clean_name.clone())
                            .await?;
                        let created_at = created.created_at;
                        let updated_at = created.updated_at;
                        let id = created.id;
                        by_name.insert(clean_name.clone(), created);
                        stats.folders_imported += 1;
                        (id, Some(created_at), Some(updated_at))
                    }
                };
                remember_folder_times(
                    folders,
                    folder_id,
                    &entry.path(),
                    existing_created,
                    existing_updated,
                )
                .await;
                stack.push((Some(folder_id), entry.path(), child_rel));
            } else if ft.is_file() {
                let Ok(clean_name) = validate_name(raw_name, FILE_MAX) else {
                    continue;
                };
                if clean_name != raw_name {
                    continue;
                }
                let child_rel = format!("{rel_dir}/{clean_name}");
                let path = entry.path();
                if let Some(existing) = files.get_by_storage_key(&child_rel).await? {
                    remember_file_disk(
                        files,
                        existing.id,
                        &clean_name,
                        existing.mime.as_deref(),
                        &path,
                        Some(existing.created_at),
                        Some(existing.updated_at),
                        existing.duracao_ms,
                    )
                    .await;
                } else {
                    let Ok(meta) = entry.metadata().await else {
                        continue;
                    };
                    let mime = guess_mime(&clean_name);
                    let created = files
                        .create(
                            user.id,
                            parent_id.map(|id| id.0),
                            clean_name.clone(),
                            mime.clone(),
                            meta.len() as i64,
                            child_rel,
                        )
                        .await?;
                    remember_file_disk(
                        files,
                        created.id,
                        &clean_name,
                        mime.as_deref(),
                        &path,
                        Some(created.created_at),
                        Some(created.updated_at),
                        None,
                    )
                    .await;
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

async fn remember_folder_times<D: FolderRepoBd>(
    folders: &D,
    id: FolderId,
    path: &Path,
    existing_created: Option<chrono::DateTime<chrono::Utc>>,
    existing_updated: Option<chrono::DateTime<chrono::Utc>>,
) {
    let Ok(meta) = tokio::fs::metadata(path).await else {
        return;
    };
    let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    let (created, updated) = resolve_timestamps(existing_created, modified, meta.created().ok());
    if timestamps_changed(existing_created, existing_updated, created, updated) {
        let _ = folders.set_timestamps(id, created, updated).await;
    }
}

async fn remember_file_disk<F: FileRepoBd>(
    files: &F,
    id: FileId,
    nome: &str,
    mime: Option<&str>,
    path: &Path,
    existing_created: Option<chrono::DateTime<chrono::Utc>>,
    existing_updated: Option<chrono::DateTime<chrono::Utc>>,
    existing_duration: Option<i64>,
) {
    if let Ok(meta) = tokio::fs::metadata(path).await {
        let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let (created, updated) = resolve_timestamps(existing_created, modified, meta.created().ok());
        if timestamps_changed(existing_created, existing_updated, created, updated) {
            let _ = files.set_timestamps(id, created, updated).await;
        }
    }
    if existing_duration.is_none() && classify_file(nome, mime) == FileCategory::Video {
        let path = path.to_path_buf();
        let nome = nome.to_string();
        let probed = tokio::task::spawn_blocking(move || {
            let mut file = File::open(path).ok()?;
            video_duration_ms(&nome, &mut file)
        })
        .await
        .ok()
        .flatten();
        if let Some(ms) = probed {
            let _ = files.set_duration(id, Some(ms)).await;
        }
    }
}

fn timestamps_changed(
    existing_created: Option<chrono::DateTime<chrono::Utc>>,
    existing_updated: Option<chrono::DateTime<chrono::Utc>>,
    created: chrono::DateTime<chrono::Utc>,
    updated: chrono::DateTime<chrono::Utc>,
) -> bool {
    existing_created
        .map(|current| !same_second(current, created))
        .unwrap_or(true)
        || existing_updated
            .map(|current| !same_second(current, updated))
            .unwrap_or(true)
}
