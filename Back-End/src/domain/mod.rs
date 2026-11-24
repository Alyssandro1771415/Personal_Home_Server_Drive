//! Regras e tipos do negócio (User, arquivo, erros).
//! Sem Axum, SQL nem sistema de arquivos.
pub mod disk_time;
pub mod duration;
pub mod error;
pub mod files;
pub mod user_secret;
pub mod users;

pub use disk_time::{resolve_timestamps, same_second};
pub use duration::video_duration_ms;
pub use error::Erro;
pub use files::{
    classify_file, dedupe_name, guess_mime, matches_search, normalize_search, validate_name,
    DiskStats, EntryKindFilter, FileCategory, FileId, Folder, FolderId, StoredFile,
    UploadConflictMode, TRASH_DIR_NAME,
};
pub use user_secret::UserSecret;
pub use users::{User, UserId};
