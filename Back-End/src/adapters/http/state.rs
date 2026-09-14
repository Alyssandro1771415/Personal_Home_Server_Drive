use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::adapters::db::postgres_users::PostgresUsers;
use crate::domain::UserId;

#[derive(Clone)]
pub struct AppState {
    pub users: PostgresUsers,
    pub sessions: Arc<Mutex<HashMap<String, UserId>>>,
}
