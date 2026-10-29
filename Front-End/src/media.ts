import type { DriveFile } from "./api";

export type FileKind = "image" | "video" | "other";

const IMAGE_EXT = ["jpg", "jpeg", "png", "gif", "webp", "bmp", "avif", "svg"];
const VIDEO_EXT = ["mp4", "webm", "ogg", "mov", "mkv", "m4v"];

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
  if (IMAGE_EXT.includes(ext)) {
    return "image";
  }
  if (VIDEO_EXT.includes(ext)) {
    return "video";
  }
  return "other";
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
