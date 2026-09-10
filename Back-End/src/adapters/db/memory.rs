use std::sync::{Arc, Mutex};

use crate::domain::{Erro, User, UserId};
use crate::ports::UserRepo;

#[derive(Clone)]
pub struct InMemoryUsers {
    inner: Arc<Mutex<Vec<User>>>,
}

impl InMemoryUsers {
    pub fn with_seed() -> Self {
        let users = vec![
            User {
                id: UserId(1),
                name: "Alice".to_string(),
            },
            User {
                id: UserId(2),
                name: "Bob".to_string(),
            },
        ];
        Self {
            inner: Arc::new(Mutex::new(users)),
        }
    }
}

impl UserRepo for InMemoryUsers {
    fn list(&self) -> Result<Vec<User>, Erro> {
        let guard = self.inner.lock().expect("mutex");
        Ok(guard.clone())
    }
}
