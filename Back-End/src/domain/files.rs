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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileCategory {
    Image,
    Video,
    Audio,
    Document,
    Archive,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKindFilter {
    All,
    Folder,
    File(FileCategory),
}

impl EntryKindFilter {
    pub fn parse(raw: Option<&str>) -> Result<Self, Erro> {
        let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok(Self::All);
        };
        match s.to_ascii_lowercase().as_str() {
            "all" => Ok(Self::All),
            "folder" => Ok(Self::Folder),
            "image" => Ok(Self::File(FileCategory::Image)),
            "video" => Ok(Self::File(FileCategory::Video)),
            "audio" => Ok(Self::File(FileCategory::Audio)),
            "document" => Ok(Self::File(FileCategory::Document)),
            "archive" => Ok(Self::File(FileCategory::Archive)),
            "other" => Ok(Self::File(FileCategory::Other)),
            _ => Err(Erro::Validacao),
        }
    }
}

const IMAGE_EXT: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "avif", "svg", "ico", "heic", "heif", "tiff",
    "tif", "raw", "cr2", "nef", "arw", "dng", "jfif",
];

const VIDEO_EXT: &[&str] = &[
    "mp4", "webm", "ogg", "mov", "mkv", "m4v", "avi", "wmv", "flv", "mpeg", "mpg", "3gp", "mts",
    "m2ts", "vob",
];

const AUDIO_EXT: &[&str] = &[
    "mp3", "wav", "flac", "aac", "m4a", "oga", "opus", "wma", "aiff", "alac", "mid", "midi",
    "amr",
];

const DOCUMENT_EXT: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "epub",
    "mobi", "txt", "md", "markdown", "csv", "tsv", "log", "json", "xml", "yaml", "yml", "toml",
    "ini", "cfg", "conf", "html", "htm", "css", "js", "jsx", "tsx", "py", "rs", "go", "java", "c",
    "cpp", "h", "sh", "sql", "tex",
];

const ARCHIVE_EXT: &[&str] = &[
    "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "tbz2", "xz", "txz", "zst", "lz4", "cab",
    "iso", "dmg", "img",
];

const BINARY_PACKAGE_EXT: &[&str] = &[
    "apk", "ipa", "jar", "war", "exe", "msi", "deb", "rpm", "appimage", "bin", "dll", "so",
];

/// Classifica um arquivo em uma das categorias de filtro da aplicação.
pub fn classify_file(nome: &str, mime: Option<&str>) -> FileCategory {
    let mime_base = mime
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();

    if mime_base.starts_with("image/") {
        return FileCategory::Image;
    }
    if mime_base.starts_with("video/") {
        return FileCategory::Video;
    }
    if mime_base.starts_with("audio/") {
        return FileCategory::Audio;
    }

    let ext = nome
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();

    if !ext.is_empty() {
        if IMAGE_EXT.contains(&ext.as_str()) {
            return FileCategory::Image;
        }
        if VIDEO_EXT.contains(&ext.as_str()) {
            return FileCategory::Video;
        }
        if AUDIO_EXT.contains(&ext.as_str()) {
            return FileCategory::Audio;
        }
        if DOCUMENT_EXT.contains(&ext.as_str()) {
            return FileCategory::Document;
        }
        if ARCHIVE_EXT.contains(&ext.as_str()) {
            return FileCategory::Archive;
        }
        if BINARY_PACKAGE_EXT.contains(&ext.as_str()) {
            return FileCategory::Other;
        }
    }

    if mime_base.starts_with("text/")
        || matches!(
            mime_base.as_str(),
            "application/pdf"
                | "application/msword"
                | "application/vnd.ms-excel"
                | "application/vnd.ms-powerpoint"
                | "application/rtf"
                | "application/json"
                | "application/xml"
                | "application/epub+zip"
        )
        || mime_base.starts_with("application/vnd.openxmlformats-officedocument.")
        || mime_base.starts_with("application/vnd.oasis.opendocument.")
    {
        return FileCategory::Document;
    }

    if matches!(
        mime_base.as_str(),
        "application/zip"
            | "application/x-zip-compressed"
            | "application/gzip"
            | "application/x-gzip"
            | "application/x-tar"
            | "application/x-7z-compressed"
            | "application/vnd.rar"
            | "application/x-rar-compressed"
            | "application/x-bzip"
            | "application/x-bzip2"
            | "application/x-xz"
            | "application/zstd"
            | "application/x-iso9660-image"
    ) {
        return FileCategory::Archive;
    }

    FileCategory::Other
}

/// Normaliza texto para busca insensível a maiúsculas/minúsculas e acentos (NFC e NFD).
pub fn normalize_search(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.trim().chars() {
        if ('\u{0300}'..='\u{036f}').contains(&ch) {
            continue;
        }
        for lower in ch.to_lowercase() {
            if ('\u{0300}'..='\u{036f}').contains(&lower) {
                continue;
            }
            let folded = match lower {
                'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'í' | 'ì' | 'î' | 'ï' => 'i',
                'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
                'ú' | 'ù' | 'û' | 'ü' => 'u',
                'ç' => 'c',
                'ñ' => 'n',
                'ý' | 'ÿ' => 'y',
                other => other,
            };
            out.push(folded);
        }
    }
    out
}

/// Verifica se `nome` contém o termo `normalized_query` (já passado por `normalize_search`).
pub fn matches_search(nome: &str, normalized_query: &str) -> bool {
    if normalized_query.is_empty() {
        return true;
    }
    normalize_search(nome).contains(normalized_query)
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

    #[test]
    fn classifies_files_across_categories() {
        assert_eq!(classify_file("foto.heic", None), FileCategory::Image);
        assert_eq!(
            classify_file("sem_ext", Some("image/png")),
            FileCategory::Image
        );
        assert_eq!(classify_file("filme.mkv", None), FileCategory::Video);
        assert_eq!(
            classify_file("voz.ogg", Some("audio/ogg")),
            FileCategory::Audio
        );
        assert_eq!(classify_file("musica.flac", None), FileCategory::Audio);
        assert_eq!(
            classify_file("relatorio.docx", Some("application/octet-stream")),
            FileCategory::Document
        );
        assert_eq!(
            classify_file("livro.epub", Some("application/zip")),
            FileCategory::Document
        );
        assert_eq!(
            classify_file("notas", Some("text/plain; charset=utf-8")),
            FileCategory::Document
        );
        assert_eq!(classify_file("backup.tar.gz", None), FileCategory::Archive);
        assert_eq!(classify_file("pacote.7z", None), FileCategory::Archive);
        assert_eq!(
            classify_file("app.apk", Some("application/zip")),
            FileCategory::Other
        );
        assert_eq!(
            classify_file("binario.bin", Some("application/octet-stream")),
            FileCategory::Other
        );
    }

    #[test]
    fn parses_entry_kind_filter() {
        assert_eq!(EntryKindFilter::parse(None).unwrap(), EntryKindFilter::All);
        assert_eq!(
            EntryKindFilter::parse(Some("  ")).unwrap(),
            EntryKindFilter::All
        );
        assert_eq!(
            EntryKindFilter::parse(Some("folder")).unwrap(),
            EntryKindFilter::Folder
        );
        assert_eq!(
            EntryKindFilter::parse(Some("DOCUMENT")).unwrap(),
            EntryKindFilter::File(FileCategory::Document)
        );
        assert!(EntryKindFilter::parse(Some("desconhecido")).is_err());
    }

    #[test]
    fn normalizes_accents_and_case_for_search() {
        let q = normalize_search("  Relatorio  ");
        assert!(matches_search("Relatório_Financeiro_2026.pdf", &q));
        assert!(matches_search("relato\u{0301}rio_mac.pdf", &q));
        assert!(!matches_search("Fatura_2026.pdf", &q));

        let q_cedilla = normalize_search("cancao");
        assert!(matches_search("Canção Nova.mp3", &q_cedilla));
    }
}
