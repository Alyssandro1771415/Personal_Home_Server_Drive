use crate::domain::{Erro, User};

pub trait UserRepo: Send + Sync {
    fn list(&self) -> Result<Vec<User>, Erro>;
}
