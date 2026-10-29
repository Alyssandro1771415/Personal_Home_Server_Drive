use chrono::{DateTime, Utc};

use super::error::Erro;
use super::users::UserId;

pub const TRASH_DIR_NAME: &str = ".lixeira";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileId(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FolderId(pub i64);

#[derive(Clone, Debug)]
pub struct StoredFile {
    pub id: FileId,
    pub owner_id: UserId,
    pub folder_id: Option<i64>,
    pub nome: String,
    pub mime: Option<String>,
    pub tamanho_bytes: i64,
    pub storage_key: String,
    pub trashed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct Folder {
    pub id: FolderId,
    pub owner_id: UserId,
    pub parent_id: Option<FolderId>,
    pub nome: String,
    pub trashed_at: Option<DateTime<Utc>>,
}

/// Valida um segmento de nome (usuário, pasta ou arquivo) para uso direto no disco.
pub fn validate_name(raw: &str, max_len: usize) -> Result<String, Erro> {
    let nome = raw.trim();
    if nome.is_empty() || nome.len() > max_len {
        return Err(Erro::Validacao);
    }
    if nome == "." || nome == ".." || nome.eq_ignore_ascii_case(TRASH_DIR_NAME) {
        return Err(Erro::Validacao);
    }
    if nome
        .chars()
        .any(|c| c == '/' || c == '\\' || c == '\0' || c.is_control())
    {
        return Err(Erro::Validacao);
    }
    Ok(nome.to_string())
}

/// Gera variação `"nome (1).ext"` caso o nome já exista no diretório de destino.
pub fn dedupe_name(nome: &str, attempt: usize, max_len: usize) -> String {
    let suffix_num = format!(" ({attempt})");
    let (stem, ext) = match nome.rfind('.') {
        Some(idx) if idx > 0 && idx < nome.len() - 1 => (&nome[..idx], &nome[idx..]),
        _ => (nome, ""),
    };

    let full = format!("{stem}{suffix_num}{ext}");
    if full.len() <= max_len {
        return full;
    }

    let overhead = suffix_num.len() + ext.len();
    let keep = max_len.saturating_sub(overhead);
    let mut trimmed = String::new();
    for ch in stem.chars() {
        if trimmed.len() + ch.len_utf8() > keep {
            break;
        }
        trimmed.push(ch);
    }
    format!("{trimmed}{suffix_num}{ext}")
}

/// Infere o MIME básico pela extensão ao sincronizar arquivos existentes no disco.
pub fn guess_mime(nome: &str) -> Option<String> {
    let ext = nome.rsplit_once('.')?.1.to_ascii_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "ogg" => "video/ogg",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "pdf" => "application/pdf",
        "txt" | "log" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        "json" => "application/json",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "tar" => "application/x-tar",
        _ => return None,
    };
    Some(mime.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_safe_names() {
        assert_eq!(validate_name("  Fotos 2026  ", 80).unwrap(), "Fotos 2026");
        assert!(validate_name("", 80).is_err());
        assert!(validate_name("..", 80).is_err());
        assert!(validate_name(".", 80).is_err());
        assert!(validate_name(".lixeira", 80).is_err());
        assert!(validate_name("a/b", 80).is_err());
        assert!(validate_name("a\\b", 80).is_err());
    }

    #[test]
    fn dedupes_preserving_extension() {
        assert_eq!(dedupe_name("foto.jpg", 1, 200), "foto (1).jpg");
        assert_eq!(dedupe_name("README", 2, 200), "README (2)");
        assert_eq!(dedupe_name(".gitignore", 1, 200), ".gitignore (1)");
    }

    #[test]
    fn guesses_common_mimes() {
        assert_eq!(guess_mime("Foto.JPG").as_deref(), Some("image/jpeg"));
        assert_eq!(guess_mime("video.mp4").as_deref(), Some("video/mp4"));
        assert_eq!(guess_mime("sem_extensao"), None);
    }
}
