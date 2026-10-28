import { createFolder, listFolders, type DriveFolder } from "./api";

const NOME_MAX = 80;

const SKIP = new Set([".ds_store", "thumbs.db", "desktop.ini", "__macosx"]);

export type UploadItem = {
  file?: File;
  dirs: string[];
};

function skipName(name: string): boolean {
  return SKIP.has(name.trim().toLowerCase());
}

function sanitizeSegment(raw: string): string | null {
  const nome = raw.trim().replaceAll("/", "").replaceAll("\\", "");
  if (!nome || skipName(nome)) {
    return null;
  }
  return nome.slice(0, NOME_MAX);
}

function dirsFromRelative(rel: string): string[] | null {
  const parts = rel.split(/[/\\]/).filter(Boolean);
  parts.pop();
  const dirs: string[] = [];
  for (const part of parts) {
    if (skipName(part)) {
      return null;
    }
    const clean = sanitizeSegment(part);
    if (clean) {
      dirs.push(clean);
    }
  }
  return dirs;
}

export function itemsFromFileList(list: FileList | File[]): UploadItem[] {
  const items: UploadItem[] = [];
  for (const file of Array.from(list)) {
    if (skipName(file.name)) {
      continue;
    }
    const rel = "webkitRelativePath" in file ? file.webkitRelativePath : "";
    const dirs = rel ? dirsFromRelative(rel) : [];
    if (dirs === null) {
      continue;
    }
    items.push({ file, dirs });
  }
  return items;
}

type FsEntry = {
  isFile: boolean;
  isDirectory: boolean;
  name: string;
  file: (ok: (file: File) => void, err?: (error: DOMException) => void) => void;
  createReader: () => {
    readEntries: (
      ok: (entries: FsEntry[]) => void,
      err?: (error: DOMException) => void,
    ) => void;
  };
};

function asEntry(item: DataTransferItem): FsEntry | null {
  const getter = (
    item as DataTransferItem & { webkitGetAsEntry?: () => FsEntry | null }
  ).webkitGetAsEntry;
  return getter?.call(item) ?? null;
}

function readAllEntries(reader: ReturnType<FsEntry["createReader"]>): Promise<FsEntry[]> {
  return new Promise((resolve, reject) => {
    const all: FsEntry[] = [];
    const tick = () => {
      reader.readEntries((batch) => {
        if (batch.length === 0) {
          resolve(all);
          return;
        }
        all.push(...batch);
        tick();
      }, reject);
    };
    tick();
  });
}

function readFile(entry: FsEntry): Promise<File> {
  return new Promise((resolve, reject) => {
    entry.file(resolve, reject);
  });
}

async function walkEntry(entry: FsEntry, prefix: string[], out: UploadItem[]): Promise<void> {
  if (skipName(entry.name)) {
    return;
  }
  if (entry.isFile) {
    const file = await readFile(entry);
    if (!skipName(file.name)) {
      out.push({ file, dirs: prefix });
    }
    return;
  }
  if (!entry.isDirectory) {
    return;
  }
  const nome = sanitizeSegment(entry.name);
  if (!nome) {
    return;
  }
  const next = [...prefix, nome];
  const children = await readAllEntries(entry.createReader());
  if (children.length === 0) {
    out.push({ dirs: next });
    return;
  }
  for (const child of children) {
    await walkEntry(child, next, out);
  }
}

export async function itemsFromDataTransfer(data: DataTransfer): Promise<UploadItem[]> {
  const out: UploadItem[] = [];
  const entries: FsEntry[] = [];
  for (const item of Array.from(data.items)) {
    const entry = asEntry(item);
    if (entry) {
      entries.push(entry);
    }
  }
  if (entries.length > 0) {
    for (const entry of entries) {
      await walkEntry(entry, [], out);
    }
    return out;
  }
  return itemsFromFileList(data.files);
}

export async function ensureFolderPath(
  rootId: number | null,
  dirs: string[],
  cache: Map<string, DriveFolder[]>,
): Promise<number | null> {
  let current = rootId;
  for (const nome of dirs) {
    const key = current === null ? "root" : String(current);
    let siblings = cache.get(key);
    if (!siblings) {
      siblings = await listFolders(current);
      cache.set(key, siblings);
    }
    let folder = siblings.find((item) => item.nome === nome);
    if (!folder) {
      folder = await createFolder(nome, current);
      siblings.push(folder);
    }
    current = folder.id;
  }
  return current;
}
