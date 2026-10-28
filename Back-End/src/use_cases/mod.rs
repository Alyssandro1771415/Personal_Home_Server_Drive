//! Casos de uso: listar usuários, login, arquivos, trocar senha.
//! Falam com o domínio e com as portas, nunca com Axum ou SQLite direto.
//! Trocar senha precisa re-embrulhar a DEK; sem isso os arquivos ficam ilegíveis.
pub mod download_file;
pub mod list_root_files;
pub mod list_users_bd;
pub mod login;
pub mod register;
pub mod upload_file;
