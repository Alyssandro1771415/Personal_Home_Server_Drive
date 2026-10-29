use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::{
    classify_file, matches_search, normalize_search, EntryKindFilter, Erro, Folder, FolderId,
    StoredFile, UserId,
};
use crate::ports::{FileRepoBd, FolderRepoBd};

#[derive(Clone, Debug)]
pub struct LocatedFolder {
    pub folder: Folder,
    pub location: String,
}

#[derive(Clone, Debug)]
pub struct LocatedFile {
    pub file: StoredFile,
    pub location: String,
}

#[derive(Clone, Debug)]
pub struct SearchListing {
    pub folders: Vec<LocatedFolder>,
    pub files: Vec<LocatedFile>,
}

pub struct SearchDrive<F, D> {
    pub files: Arc<F>,
    pub folders: Arc<D>,
}

fn build_location(parent_id: Option<i64>, by_id: &HashMap<i64, Folder>) -> Option<String> {
    let Some(mut cur) = parent_id else {
        return Some("Início".to_string());
    };
    let mut parts: Vec<&str> = Vec::new();
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 32 {
            return None;
        }
        let folder = by_id.get(&cur)?;
        parts.push(&folder.nome);
        match folder.parent_id {
            Some(next) => cur = next.0,
            None => break,
        }
    }
    parts.reverse();
    Some(parts.join(" / "))
}

fn is_in_subtree(parent_id: Option<i64>, root_id: i64, by_id: &HashMap<i64, Folder>) -> bool {
    let mut cur = parent_id;
    let mut guard = 0;
    while let Some(id) = cur {
        guard += 1;
        if guard > 32 {
            return false;
        }
        if id == root_id {
            return true;
        }
        let Some(folder) = by_id.get(&id) else {
            return false;
        };
        cur = folder.parent_id.map(|p| p.0);
    }
    false
}

fn matches_scope(
    parent_id: Option<i64>,
    root_folder: Option<FolderId>,
    recursive: bool,
    by_id: &HashMap<i64, Folder>,
) -> bool {
    match (root_folder, recursive) {
        (None, true) => true,
        (None, false) => parent_id.is_none(),
        (Some(root), false) => parent_id == Some(root.0),
        (Some(root), true) => is_in_subtree(parent_id, root.0, by_id),
    }
}

pub fn filter_entries(
    active_folders: Vec<Folder>,
    active_files: Vec<StoredFile>,
    query: Option<&str>,
    kind: EntryKindFilter,
    root_folder: Option<FolderId>,
    recursive: bool,
) -> Result<SearchListing, Erro> {
    let by_id: HashMap<i64, Folder> = active_folders.iter().map(|f| (f.id.0, f.clone())).collect();

    if let Some(root) = root_folder {
        if !by_id.contains_key(&root.0) || build_location(Some(root.0), &by_id).is_none() {
            return Err(Erro::NaoEncontrado);
        }
    }

    let normalized_query = query.map(normalize_search).unwrap_or_default();
    let mut location_cache: HashMap<Option<i64>, Option<String>> = HashMap::new();

    let mut resolve_location = |parent_id: Option<i64>| -> Option<String> {
        location_cache
            .entry(parent_id)
            .or_insert_with(|| build_location(parent_id, &by_id))
            .clone()
    };

    let include_folders = matches!(kind, EntryKindFilter::All | EntryKindFilter::Folder);
    let folders = if include_folders {
        active_folders
            .into_iter()
            .filter_map(|folder| {
                let parent_id = folder.parent_id.map(|p| p.0);
                if !matches_scope(parent_id, root_folder, recursive, &by_id) {
                    return None;
                }
                if !matches_search(&folder.nome, &normalized_query) {
                    return None;
                }
                let location = resolve_location(parent_id)?;
                Some(LocatedFolder { folder, location })
            })
            .collect()
    } else {
        Vec::new()
    };

    let files = match kind {
        EntryKindFilter::Folder => Vec::new(),
        EntryKindFilter::All | EntryKindFilter::File(_) => active_files
            .into_iter()
            .filter_map(|file| {
                if !matches_scope(file.folder_id, root_folder, recursive, &by_id) {
                    return None;
                }
                if let EntryKindFilter::File(target_cat) = kind {
                    if classify_file(&file.nome, file.mime.as_deref()) != target_cat {
                        return None;
                    }
                }
                if !matches_search(&file.nome, &normalized_query) {
                    return None;
                }
                let location = resolve_location(file.folder_id)?;
                Some(LocatedFile { file, location })
            })
            .collect(),
    };

    Ok(SearchListing { folders, files })
}

impl<F: FileRepoBd, D: FolderRepoBd> SearchDrive<F, D> {
    pub async fn execute(
        &self,
        owner: UserId,
        query: Option<&str>,
        kind: EntryKindFilter,
        root_folder: Option<FolderId>,
        recursive: bool,
    ) -> Result<SearchListing, Erro> {
        let active_folders = self.folders.list_all_active(owner).await?;
        let active_files = if kind == EntryKindFilter::Folder {
            Vec::new()
        } else {
            self.files.list_all_active(owner).await?
        };

        filter_entries(
            active_folders,
            active_files,
            query,
            kind,
            root_folder,
            recursive,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FileCategory, FileId};

    fn sample_folder(id: i64, parent_id: Option<i64>, nome: &str) -> Folder {
        Folder {
            id: FolderId(id),
            owner_id: UserId(1),
            parent_id: parent_id.map(FolderId),
            nome: nome.to_string(),
            trashed_at: None,
        }
    }

    fn sample_file(id: i64, folder_id: Option<i64>, nome: &str, mime: Option<&str>) -> StoredFile {
        StoredFile {
            id: FileId(id),
            owner_id: UserId(1),
            folder_id,
            nome: nome.to_string(),
            mime: mime.map(|s| s.to_string()),
            tamanho_bytes: 1024,
            storage_key: format!("u/{id}"),
            trashed_at: None,
        }
    }

    #[test]
    fn searches_globally_with_locations_and_accents() {
        let folders = vec![
            sample_folder(10, None, "Fotos"),
            sample_folder(20, Some(10), "Férias 2026"),
        ];
        let files = vec![
            sample_file(1, None, "Relatório.pdf", Some("application/pdf")),
            sample_file(2, Some(20), "praia_verão.jpg", Some("image/jpeg")),
            sample_file(3, Some(10), "video_familia.mp4", Some("video/mp4")),
        ];

        let res = filter_entries(
            folders.clone(),
            files.clone(),
            Some("ferias"),
            EntryKindFilter::All,
            None,
            true,
        )
        .unwrap();
        assert_eq!(res.folders.len(), 1);
        assert_eq!(res.folders[0].folder.nome, "Férias 2026");
        assert_eq!(res.folders[0].location, "Fotos");
        assert_eq!(res.files.len(), 0);

        let imgs = filter_entries(
            folders,
            files,
            None,
            EntryKindFilter::File(FileCategory::Image),
            None,
            true,
        )
        .unwrap();
        assert_eq!(imgs.folders.len(), 0);
        assert_eq!(imgs.files.len(), 1);
        assert_eq!(imgs.files[0].file.nome, "praia_verão.jpg");
        assert_eq!(imgs.files[0].location, "Fotos / Férias 2026");
    }

    #[test]
    fn scopes_to_subtree_and_validates_missing_root() {
        let folders = vec![
            sample_folder(10, None, "Fotos"),
            sample_folder(20, Some(10), "2026"),
            sample_folder(30, None, "Documentos"),
        ];
        let files = vec![
            sample_file(1, Some(30), "contrato.pdf", None),
            sample_file(2, Some(20), "roteiro.pdf", None),
        ];

        let scoped = filter_entries(
            folders.clone(),
            files.clone(),
            None,
            EntryKindFilter::File(FileCategory::Document),
            Some(FolderId(10)),
            true,
        )
        .unwrap();
        assert_eq!(scoped.files.len(), 1);
        assert_eq!(scoped.files[0].file.nome, "roteiro.pdf");

        let missing = filter_entries(
            folders,
            files,
            None,
            EntryKindFilter::All,
            Some(FolderId(999)),
            true,
        );
        assert!(matches!(missing.unwrap_err(), Erro::NaoEncontrado));
    }
}
