import type { DriveFile, DriveFolder } from "./api";
import { fileCategory, FILTER_OPTIONS, type FileCategory } from "./media";

export type SortKey = "name" | "size" | "created" | "modified" | "type";
export type SortDir = "asc" | "desc";

export type SortPref = {
  key: SortKey;
  dir: SortDir;
};

const STORAGE_KEY = "home-drive-sort";

export const SORT_OPTIONS: ReadonlyArray<{ value: SortKey; label: string }> = [
  { value: "name", label: "Nome" },
  { value: "size", label: "Tamanho" },
  { value: "created", label: "Data de criação" },
  { value: "modified", label: "Data de modificação" },
  { value: "type", label: "Tipo" },
];

const CATEGORY_LABEL = new Map<FileCategory, string>(
  FILTER_OPTIONS.filter((opt) => opt.value !== "all" && opt.value !== "folder").map((opt) => [
    opt.value as FileCategory,
    opt.label,
  ]),
);

export function defaultDir(key: SortKey): SortDir {
  return key === "size" || key === "created" || key === "modified" ? "desc" : "asc";
}

export function loadSort(): SortPref {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return { key: "name", dir: "asc" };
    }
    const parsed = JSON.parse(raw) as { key?: string; dir?: string };
    const key = SORT_OPTIONS.some((opt) => opt.value === parsed.key)
      ? (parsed.key as SortKey)
      : "name";
    const dir = parsed.dir === "asc" || parsed.dir === "desc" ? parsed.dir : defaultDir(key);
    return { key, dir };
  } catch {
    return { key: "name", dir: "asc" };
  }
}

export function saveSort(pref: SortPref): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(pref));
  } catch {
    /* preferência é só conveniência */
  }
}

export function sortDirectionLabel(key: SortKey, dir: SortDir): string {
  if (key === "size") {
    return dir === "desc" ? "Maiores primeiro" : "Menores primeiro";
  }
  if (key === "created" || key === "modified") {
    return dir === "desc" ? "Mais recentes primeiro" : "Mais antigos primeiro";
  }
  return dir === "asc" ? "A até Z" : "Z até A";
}

export function formatSortDate(iso?: string): string | null {
  if (!iso) {
    return null;
  }
  const time = Date.parse(iso);
  if (Number.isNaN(time)) {
    return null;
  }
  return new Date(time).toLocaleString("pt-BR", {
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function categoryLabel(file: DriveFile): string {
  return CATEGORY_LABEL.get(fileCategory(file)) ?? "Outros";
}

function compareName(a: string, b: string): number {
  return a.localeCompare(b, "pt", { sensitivity: "base", numeric: true });
}

function timeOf(iso?: string): number {
  if (!iso) {
    return 0;
  }
  const time = Date.parse(iso);
  return Number.isNaN(time) ? 0 : time;
}

function fileTypeKey(file: DriveFile): string {
  const ext = file.nome.includes(".") ? (file.nome.split(".").pop() ?? "").toLowerCase() : "";
  return `${categoryLabel(file)} ${ext}`;
}

function withDir(diff: number, dir: SortDir): number {
  return dir === "asc" ? diff : -diff;
}

function finish(primary: number, nameA: string, nameB: string, idA: number, idB: number, dir: SortDir): number {
  let diff = primary;
  if (diff === 0) {
    diff = compareName(nameA, nameB);
  }
  if (diff === 0) {
    diff = idA - idB;
  }
  return withDir(diff, dir);
}

export function sortFolders(folders: DriveFolder[], key: SortKey, dir: SortDir): DriveFolder[] {
  const copy = [...folders];
  copy.sort((a, b) => {
    if (key === "size" || key === "type") {
      return finish(0, a.nome, b.nome, a.id, b.id, "asc");
    }
    const primary =
      key === "created"
        ? timeOf(a.created_at) - timeOf(b.created_at)
        : key === "modified"
          ? timeOf(a.updated_at) - timeOf(b.updated_at)
          : compareName(a.nome, b.nome);
    return finish(primary, a.nome, b.nome, a.id, b.id, dir);
  });
  return copy;
}

export function sortFiles(files: DriveFile[], key: SortKey, dir: SortDir): DriveFile[] {
  const copy = [...files];
  copy.sort((a, b) => {
    let primary = 0;
    if (key === "name") {
      primary = compareName(a.nome, b.nome);
    } else if (key === "size") {
      primary = a.tamanho_bytes - b.tamanho_bytes;
    } else if (key === "created") {
      primary = timeOf(a.created_at) - timeOf(b.created_at);
    } else if (key === "modified") {
      primary = timeOf(a.updated_at) - timeOf(b.updated_at);
    } else {
      primary = compareName(fileTypeKey(a), fileTypeKey(b));
    }
    return finish(primary, a.nome, b.nome, a.id, b.id, dir);
  });
  return copy;
}
