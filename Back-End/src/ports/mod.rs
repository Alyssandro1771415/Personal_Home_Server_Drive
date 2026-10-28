//! Contratos que o domínio precisa (traits).
//! Os adapters implementam estas portas.
pub mod crypto;
pub mod file_repo_bd;
pub mod file_store;
pub mod folder_repo_bd;
pub mod user_repo_bd;

pub use crypto::Crypto;
pub use file_repo_bd::FileRepoBd;
pub use file_store::FileStore;
pub use folder_repo_bd::FolderRepoBd;
pub use user_repo_bd::UserRepoBd;
