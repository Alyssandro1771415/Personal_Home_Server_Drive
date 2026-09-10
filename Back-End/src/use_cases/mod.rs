//! Casos de uso: listar usuários, login, arquivos, trocar senha.
//! Falam com o domínio e com as portas, nunca com Axum ou SQLite direto.
pub mod list_users_bd;
pub mod login;
