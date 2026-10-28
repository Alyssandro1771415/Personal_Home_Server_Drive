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
};

export type DriveFolder = {
  id: number;
  nome: string;
  parent_id: number | null;
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
