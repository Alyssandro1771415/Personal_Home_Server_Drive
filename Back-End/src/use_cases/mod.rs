//! Casos de uso: usuários, login, redefinição de senha, pastas, arquivos, lixeira e sincronização em disco.
//! Falam com o domínio e com as portas, nunca com Axum ou banco direto.
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
pub mod reset_password;
pub mod storage_paths;
pub mod sync_disk;
pub mod upload_file;
