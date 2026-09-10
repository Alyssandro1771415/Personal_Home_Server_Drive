//! Regras e tipos do negócio (User, arquivo, erros).
//! Sem Axum, SQL nem sistema de arquivos.
pub mod error;
pub mod users;

pub use error::Erro;
pub use users::{User, UserId};
