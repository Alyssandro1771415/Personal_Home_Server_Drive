//! Regras e tipos do negócio (User, arquivo, erros).
//! Sem Axum, SQL nem sistema de arquivos.
pub mod dek;
pub mod error;
pub mod files;
pub mod users;
pub mod user_secret;

pub use dek::{Dek, WrappedDek};
pub use error::Erro;
pub use files::{FileId, Folder, FolderId, StoredFile};
pub use users::{User, UserId};
pub use user_secret::UserSecret;
