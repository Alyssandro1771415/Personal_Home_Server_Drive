//! Casos de uso: listar usuários, login, arquivos, trocar senha.
//! Falam com o domínio e com as portas, nunca com Axum ou SQLite direto.
//! Trocar senha precisa re-embrulhar a DEK; sem isso os arquivos ficam ilegíveis.
pub mod create_folder;
pub mod download_file;
pub mod folder_path;
pub mod list_files;
pub mod list_folders;
pub mod list_trash;
pub mod list_users_bd;
pub mod login;
pub mod mutate_file;
pub mod mutate_folder;
pub mod purge_expired;
pub mod register;
pub mod upload_file;
