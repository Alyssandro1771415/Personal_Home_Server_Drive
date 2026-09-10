//! Contratos que o domínio precisa (traits).
//! Os adapters implementam estas portas.
pub mod user_repo_bd;
pub use user_repo_bd::UserRepoBd;