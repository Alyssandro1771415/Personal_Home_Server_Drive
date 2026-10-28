use super::users::UserId;

#[derive(Clone)]
pub struct UserSecret {
    pub user_id: UserId,
    pub password_hash: String,
    pub dek_salt: Vec<u8>,
    pub wrapped_dek: Vec<u8>,
}
