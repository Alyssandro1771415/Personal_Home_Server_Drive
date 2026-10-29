import type { DriveFile } from "./api";

export type FileKind = "image" | "video" | "other";

export type FileCategory =
  | "image"
  | "video"
  | "audio"
  | "document"
  | "archive"
  | "other";

export type EntryFilterKind = "all" | "folder" | FileCategory;

export const FILTER_OPTIONS: ReadonlyArray<{
  value: EntryFilterKind;
  label: string;
}> = [
  { value: "all", label: "Todos" },
  { value: "folder", label: "Pastas" },
  { value: "image", label: "Imagens" },
  { value: "video", label: "Vídeos" },
  { value: "audio", label: "Áudios" },
  { value: "document", label: "Documentos" },
  { value: "archive", label: "Compactados" },
  { value: "other", label: "Outros" },
];

const PREVIEW_IMAGE_EXT = ["jpg", "jpeg", "png", "gif", "webp", "bmp", "avif", "svg"];
const PREVIEW_VIDEO_EXT = ["mp4", "webm", "ogg", "mov", "mkv", "m4v"];

const IMAGE_EXT = [
  "jpg",
  "jpeg",
  "png",
  "gif",
  "webp",
  "bmp",
  "avif",
  "svg",
  "ico",
  "heic",
  "heif",
  "tiff",
  "tif",
  "raw",
  "cr2",
  "nef",
  "arw",
  "dng",
  "jfif",
];

const VIDEO_EXT = [
  "mp4",
  "webm",
  "ogg",
  "mov",
  "mkv",
  "m4v",
  "avi",
  "wmv",
  "flv",
  "mpeg",
  "mpg",
  "3gp",
  "mts",
  "m2ts",
  "vob",
];

const AUDIO_EXT = [
  "mp3",
  "wav",
  "flac",
  "aac",
  "m4a",
  "oga",
  "opus",
  "wma",
  "aiff",
  "alac",
  "mid",
  "midi",
  "amr",
];

const DOCUMENT_EXT = [
  "pdf",
  "doc",
  "docx",
  "xls",
  "xlsx",
  "ppt",
  "pptx",
  "odt",
  "ods",
  "odp",
  "rtf",
  "epub",
  "mobi",
  "txt",
  "md",
  "markdown",
  "csv",
  "tsv",
  "log",
  "json",
  "xml",
  "yaml",
  "yml",
  "toml",
  "ini",
  "cfg",
  "conf",
  "html",
  "htm",
  "css",
  "js",
  "jsx",
  "tsx",
  "py",
  "rs",
  "go",
  "java",
  "c",
  "cpp",
  "h",
  "sh",
  "sql",
  "tex",
];

const ARCHIVE_EXT = [
  "zip",
  "rar",
  "7z",
  "tar",
  "gz",
  "tgz",
  "bz2",
  "tbz2",
  "xz",
  "txz",
  "zst",
  "lz4",
  "cab",
  "iso",
  "dmg",
  "img",
];

const BINARY_PACKAGE_EXT = [
  "apk",
  "ipa",
  "jar",
  "war",
  "exe",
  "msi",
  "deb",
  "rpm",
  "appimage",
  "bin",
  "dll",
  "so",
];

const DOCUMENT_MIMES = new Set([
  "application/pdf",
  "application/msword",
  "application/vnd.ms-excel",
  "application/vnd.ms-powerpoint",
  "application/rtf",
  "application/json",
  "application/xml",
  "application/epub+zip",
]);

const ARCHIVE_MIMES = new Set([
  "application/zip",
  "application/x-zip-compressed",
  "application/gzip",
  "application/x-gzip",
  "application/x-tar",
  "application/x-7z-compressed",
  "application/vnd.rar",
  "application/x-rar-compressed",
  "application/x-bzip",
  "application/x-bzip2",
  "application/x-xz",
  "application/zstd",
  "application/x-iso9660-image",
]);

export function extensionOf(nome: string): string {
  const dot = nome.lastIndexOf(".");
  if (dot < 0) {
    return "";
  }
  return nome.slice(dot + 1).toLowerCase();
}

export function fileKind(file: DriveFile): FileKind {
  const mime = file.mime?.toLowerCase() ?? "";
  if (mime.startsWith("image/")) {
    return "image";
  }
  if (mime.startsWith("video/")) {
    return "video";
  }
  const ext = extensionOf(file.nome);
  if (PREVIEW_IMAGE_EXT.includes(ext)) {
    return "image";
  }
  if (PREVIEW_VIDEO_EXT.includes(ext)) {
    return "video";
  }
  return "other";
}

export function fileCategory(file: DriveFile): FileCategory {
  const mimeBase = (file.mime ?? "").split(";")[0].trim().toLowerCase();

  if (mimeBase.startsWith("image/")) {
    return "image";
  }
  if (mimeBase.startsWith("video/")) {
    return "video";
  }
  if (mimeBase.startsWith("audio/")) {
    return "audio";
  }

  const ext = extensionOf(file.nome);
  if (ext) {
    if (IMAGE_EXT.includes(ext)) {
      return "image";
    }
    if (VIDEO_EXT.includes(ext)) {
      return "video";
    }
    if (AUDIO_EXT.includes(ext)) {
      return "audio";
    }
    if (DOCUMENT_EXT.includes(ext)) {
      return "document";
    }
    if (ARCHIVE_EXT.includes(ext)) {
      return "archive";
    }
    if (BINARY_PACKAGE_EXT.includes(ext)) {
      return "other";
    }
  }

  if (
    mimeBase.startsWith("text/") ||
    DOCUMENT_MIMES.has(mimeBase) ||
    mimeBase.startsWith("application/vnd.openxmlformats-officedocument.") ||
    mimeBase.startsWith("application/vnd.oasis.opendocument.")
  ) {
    return "document";
  }

  if (ARCHIVE_MIMES.has(mimeBase)) {
    return "archive";
  }

  return "other";
}

export function normalizeSearch(raw: string): string {
  return raw
    .trim()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
}

export function matchesSearch(nome: string, normalizedQuery: string): boolean {
  if (!normalizedQuery) {
    return true;
  }
  return normalizeSearch(nome).includes(normalizedQuery);
}

export function canAutoThumb(file: DriveFile): boolean {
  const kind = fileKind(file);
  if (kind === "image") {
    return file.tamanho_bytes <= 12 * 1024 * 1024;
  }
  if (kind === "video") {
    return file.tamanho_bytes <= 8 * 1024 * 1024;
  }
  return false;
}

export function formatBytes(n: number): string {
  if (n < 1024) {
    return `${n} B`;
  }
  const units = ["KB", "MB", "GB"];
  let value = n / 1024;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[i]}`;
}

export function formatTransferBytes(bytes: number): string {
  const safe = Math.max(0, Math.floor(bytes));
  if (safe < 1024) {
    return `${safe} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let value = safe / 1024;
  let i = 0;
  while (i < units.length - 1) {
    const decimals = i === 0 ? (value >= 10 ? 0 : 1) : i === 1 ? 1 : 2;
    const factor = 10 ** decimals;
    if (Math.round(value * factor) / factor < 1024) {
      break;
    }
    value /= 1024;
    i += 1;
  }
  const decimals = i === 0 ? (value >= 10 ? 0 : 1) : i === 1 ? 1 : 2;
  return `${value.toFixed(decimals)} ${units[i]}`;
}
