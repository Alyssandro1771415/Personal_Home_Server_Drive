const TOKEN_KEY = "home-drive-token";

export type User = {
  id: number;
  name: string;
};

export type DriveFile = {
  id: number;
  nome: string;
  mime: string | null;
  tamanho_bytes: number;
  folder_id: number | null;
  trashed_at?: string;
  expires_at?: string;
};

export type DriveFolder = {
  id: number;
  nome: string;
  parent_id: number | null;
  trashed_at?: string;
  expires_at?: string;
};

export type TrashListing = {
  folders: DriveFolder[];
  files: DriveFile[];
  ttl_days: number;
};

export class ApiError extends Error {
  status: number;

  constructor(status: number, message: string) {
    super(message);
    this.status = status;
    this.name = "ApiError";
  }
}

export function getToken(): string | null {
  return sessionStorage.getItem(TOKEN_KEY);
}

export function setToken(token: string | null): void {
  if (token) {
    sessionStorage.setItem(TOKEN_KEY, token);
  } else {
    sessionStorage.removeItem(TOKEN_KEY);
  }
}

async function parseError(res: Response): Promise<ApiError> {
  if (res.status === 401) {
    return new ApiError(401, "Não autorizado.");
  }
  if (res.status === 404) {
    return new ApiError(404, "Não encontrado.");
  }
  if (res.status === 409) {
    return new ApiError(409, "Esse nome já está em uso.");
  }
  if (res.status === 400) {
    return new ApiError(400, "Dados inválidos.");
  }
  return new ApiError(res.status, "Erro no servidor.");
}

function apiUrl(path: string): string {
  return `/api${path}`;
}

async function request(path: string, init: RequestInit = {}): Promise<Response> {
  const headers = new Headers(init.headers);
  const token = getToken();
  if (token) {
    headers.set("Authorization", `Bearer ${token}`);
  }
  const res = await fetch(apiUrl(path), { ...init, headers });
  if (res.status === 401) {
    setToken(null);
  }
  return res;
}

async function expectOk(res: Response): Promise<void> {
  if (!res.ok) {
    throw await parseError(res);
  }
}

export async function register(nome: string, password: string): Promise<{ id: number }> {
  const res = await request("/users", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ nome, password }),
  });
  if (!res.ok) {
    throw await parseError(res);
  }
  return res.json() as Promise<{ id: number }>;
}

export async function login(nome: string, password: string): Promise<string> {
  const res = await request("/login", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ nome, password }),
  });
  const body = (await res.json()) as { token?: string; message?: string };
  if (!res.ok || !body.token) {
    throw new ApiError(res.status, body.message ?? "Nome ou senha inválidos.");
  }
  setToken(body.token);
  return body.token;
}

export async function me(): Promise<User> {
  const res = await request("/me");
  if (!res.ok) {
    throw await parseError(res);
  }
  return res.json() as Promise<User>;
}

export async function logout(): Promise<void> {
  try {
    await request("/logout", { method: "POST" });
  } finally {
    setToken(null);
  }
}

function folderQuery(folderId: number | null): string {
  return folderId === null ? "" : `?folder_id=${folderId}`;
}

function parentQuery(parentId: number | null): string {
  return parentId === null ? "" : `?parent_id=${parentId}`;
}

export async function listFiles(folderId: number | null): Promise<DriveFile[]> {
  const res = await request(`/files${folderQuery(folderId)}`);
  if (!res.ok) {
    throw await parseError(res);
  }
  return res.json() as Promise<DriveFile[]>;
}

export async function listFolders(parentId: number | null): Promise<DriveFolder[]> {
  const res = await request(`/folders${parentQuery(parentId)}`);
  if (!res.ok) {
    throw await parseError(res);
  }
  return res.json() as Promise<DriveFolder[]>;
}

export async function folderPath(id: number): Promise<DriveFolder[]> {
  const res = await request(`/folders/${id}/path`);
  if (!res.ok) {
    throw await parseError(res);
  }
  return res.json() as Promise<DriveFolder[]>;
}

export async function createFolder(
  nome: string,
  parentId: number | null,
): Promise<DriveFolder> {
  const res = await request("/folders", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ nome, parent_id: parentId }),
  });
  if (!res.ok) {
    throw await parseError(res);
  }
  return res.json() as Promise<DriveFolder>;
}

export async function uploadFile(
  file: File,
  folderId: number | null,
): Promise<DriveFile> {
  const body = new FormData();
  body.append("file", file);
  if (folderId !== null) {
    body.append("folder_id", String(folderId));
  }
  const res = await request("/files", { method: "POST", body });
  if (!res.ok) {
    throw await parseError(res);
  }
  return res.json() as Promise<DriveFile>;
}

export async function fetchFileBlob(id: number): Promise<Blob> {
  const res = await request(`/files/${id}/content`);
  if (!res.ok) {
    throw await parseError(res);
  }
  return res.blob();
}

export async function downloadFile(id: number, nome: string): Promise<void> {
  const blob = await fetchFileBlob(id);
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = nome;
  a.click();
  URL.revokeObjectURL(url);
}

export async function renameFile(id: number, nome: string): Promise<void> {
  const res = await request(`/files/${id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ nome }),
  });
  await expectOk(res);
}

export async function moveFile(id: number, folderId: number | null): Promise<void> {
  const res = await request(`/files/${id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ folder_id: folderId }),
  });
  await expectOk(res);
}

export async function trashFile(id: number): Promise<void> {
  await expectOk(await request(`/files/${id}`, { method: "DELETE" }));
}

export async function restoreFile(id: number): Promise<void> {
  await expectOk(await request(`/files/${id}/restore`, { method: "POST" }));
}

export async function purgeFile(id: number): Promise<void> {
  await expectOk(await request(`/files/${id}/purge`, { method: "DELETE" }));
}

export async function renameFolder(id: number, nome: string): Promise<void> {
  const res = await request(`/folders/${id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ nome }),
  });
  await expectOk(res);
}

export async function moveFolder(id: number, parentId: number | null): Promise<void> {
  const res = await request(`/folders/${id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ parent_id: parentId }),
  });
  await expectOk(res);
}

export async function trashFolder(id: number): Promise<void> {
  await expectOk(await request(`/folders/${id}`, { method: "DELETE" }));
}

export async function restoreFolder(id: number): Promise<void> {
  await expectOk(await request(`/folders/${id}/restore`, { method: "POST" }));
}

export async function purgeFolder(id: number): Promise<void> {
  await expectOk(await request(`/folders/${id}/purge`, { method: "DELETE" }));
}

export async function listTrash(): Promise<TrashListing> {
  const res = await request("/trash");
  if (!res.ok) {
    throw await parseError(res);
  }
  return res.json() as Promise<TrashListing>;
}

export function daysLeft(expiresAt?: string): number | null {
  if (!expiresAt) {
    return null;
  }
  const ms = new Date(expiresAt).getTime() - Date.now();
  return Math.max(0, Math.ceil(ms / 86_400_000));
}
