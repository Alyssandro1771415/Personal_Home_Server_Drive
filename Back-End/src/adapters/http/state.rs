use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::adapters::crypto::AesGcmCrypto;
use crate::adapters::db::postgres_files::PostgresFiles;
use crate::adapters::db::postgres_folders::PostgresFolders;
use crate::adapters::db::postgres_users::PostgresUsers;
use crate::adapters::fs::FsStore;
use crate::domain::{Dek, UserId};

#[derive(Clone)]
pub struct Session {
    pub user_id: UserId,
    pub dek: Dek,
}

#[derive(Clone)]
pub struct AppState {
    pub users: PostgresUsers,
    pub files: PostgresFiles,
    pub folders: PostgresFolders,
    pub store: FsStore,
    pub crypto: AesGcmCrypto,
    pub sessions: Arc<Mutex<HashMap<String, Session>>>,
}
