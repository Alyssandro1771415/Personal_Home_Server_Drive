#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserId(pub i64);

#[derive(Clone, Debug)]
pub struct User {
    pub id: UserId,
    pub name: String,
}
