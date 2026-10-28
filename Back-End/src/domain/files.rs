use super::users::UserId;

#[derive(Clone, Copy)]
pub struct FileId(pub i64);

#[derive(Clone)]
pub struct StoredFile {
    pub id: FileId,
    pub owner_id: UserId,
    pub folder_id: Option<i64>,
    pub nome: String,
    pub mime: Option<String>,
    pub tamanho_bytes: i64,
    pub storage_key: String,
}
