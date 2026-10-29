//! Regras e tipos do negócio (User, arquivo, erros).
//! Sem Axum, SQL nem sistema de arquivos.
pub mod error;
pub mod files;
pub mod user_secret;
pub mod users;

pub use error::Erro;
pub use files::{
    classify_file, dedupe_name, guess_mime, matches_search, normalize_search, validate_name,
    EntryKindFilter, FileCategory, FileId, Folder, FolderId, StoredFile, TRASH_DIR_NAME,
};
pub use user_secret::UserSecret;
pub use users::{User, UserId};
