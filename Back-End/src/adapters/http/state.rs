use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Duration;

use crate::adapters::db::postgres_files::PostgresFiles;
use crate::adapters::db::postgres_folders::PostgresFolders;
use crate::adapters::db::postgres_users::PostgresUsers;
use crate::adapters::fs::FsStore;
use crate::domain::UserId;

#[derive(Clone)]
pub struct Session {
    pub user_id: UserId,
}

#[derive(Clone)]
pub struct AppState {
    pub users: PostgresUsers,
    pub files: PostgresFiles,
    pub folders: PostgresFolders,
    pub store: FsStore,
    pub sessions: Arc<Mutex<HashMap<String, Session>>>,
    pub trash_ttl: Duration,
}
