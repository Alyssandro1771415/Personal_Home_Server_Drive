use super::users::UserId;

#[derive(Clone)]
pub struct UserSecret {
    pub user_id: UserId,
    pub password_hash: String,
}
