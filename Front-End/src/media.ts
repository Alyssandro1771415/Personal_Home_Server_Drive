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
  return detectMediaKind(file.nome, file.mime);
}

export function localFileKind(file: { name: string; type?: string }): FileKind {
  return detectMediaKind(file.name, file.type ?? null);
}

function detectMediaKind(nome: string, rawMime: string | null | undefined): FileKind {
  const mime = rawMime?.toLowerCase() ?? "";
  if (mime.startsWith("image/")) {
    return "image";
  }
  if (mime.startsWith("video/")) {
    return "video";
  }
  const ext = extensionOf(nome);
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
    return file.tamanho_bytes <= 25 * 1024 * 1024;
  }
  if (kind === "video") {
    return true;
  }
  return false;
}

export function formatBytes(n: number): string {
  if (n < 1024) {
    return `${n} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let value = n / 1024;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[i]}`;
}

export function formatCapacityUnit(bytes: number): string {
  const safe = Math.max(0, bytes);
  if (safe < 1024) {
    return `${Math.floor(safe)} B`;
  }
  const units = ["KB", "MB", "GB", "TB", "PB"];
  let value = safe / 1024;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  const decimals = value >= 100 ? 0 : 1;
  const formatted = value.toFixed(decimals).replace(/\.0$/, "");
  return `${formatted} ${units[i]}`;
}

export function formatStoragePair(usedBytes: number, totalBytes: number): string {
  return `${formatCapacityUnit(usedBytes)} / ${formatCapacityUnit(totalBytes)}`;
}

export function cleanUploadFileName(raw: string): string {
  const parts = raw.split(/[/\\]/);
  const base = (parts[parts.length - 1] ?? raw).trim();
  return base.slice(0, 200);
}

export function dedupeFileName(nome: string, attempt: number, maxLen = 200): string {
  const suffixNum = ` (${attempt})`;
  const dot = nome.lastIndexOf(".");
  const hasExt = dot > 0 && dot < nome.length - 1;
  const stem = hasExt ? nome.slice(0, dot) : nome;
  const ext = hasExt ? nome.slice(dot) : "";
  const full = `${stem}${suffixNum}${ext}`;
  if (full.length <= maxLen) {
    return full;
  }
  const keep = Math.max(1, maxLen - suffixNum.length - ext.length);
  return `${stem.slice(0, keep)}${suffixNum}${ext}`;
}

export function suggestFreeFileName(
  desired: string,
  existingNames: Iterable<string>,
  maxLen = 200,
): string {
  const taken = new Set(existingNames);
  if (!taken.has(desired)) {
    return desired;
  }
  for (let attempt = 1; attempt <= 9999; attempt += 1) {
    const candidate = dedupeFileName(desired, attempt, maxLen);
    if (!taken.has(candidate)) {
      return candidate;
    }
  }
  return dedupeFileName(desired, Date.now() % 10000, maxLen);
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
