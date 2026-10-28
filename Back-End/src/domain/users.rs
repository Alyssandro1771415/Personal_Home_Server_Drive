#[derive(Clone, Copy, PartialEq, Eq)]
pub struct UserId(pub i64);

#[derive(Clone)]
pub struct User {
    pub id: UserId,
    pub name: String,
}
