export type UploadTaskStatus =
  | "queued"
  | "uploading"
  | "conflict"
  | "done"
  | "skipped"
  | "interrupted"
  | "error";

export type UploadTaskRecord = {
  id: string;
  name: string;
  finalName?: string;
  size: number;
  loadedBytes: number;
  dirs: string[];
  rootFolderId: number | null;
  isFolderPlaceholder?: boolean;
  status: UploadTaskStatus;
  resolution?: "created" | "kept_both" | "overwritten";
  errorMessage?: string;
};

export type PersistedUploadState = {
  totalUploadedCount: number;
  tasks: UploadTaskRecord[];
  updatedAt: number;
  minimized: boolean;
};

const STORAGE_PREFIX = "home-drive-upload-state-v1:";
const MAX_STORED_TASKS = 250;

const IDB_NAME = "home-drive-upload-queue-v1";
const IDB_STORE = "pending_files";

function storageKey(userId: number): string {
  return `${STORAGE_PREFIX}${userId}`;
}

export function emptyUploadState(): PersistedUploadState {
  return {
    totalUploadedCount: 0,
    tasks: [],
    updatedAt: Date.now(),
    minimized: false,
  };
}

export function loadUploadState(userId: number): PersistedUploadState {
  try {
    const raw = localStorage.getItem(storageKey(userId));
    if (!raw) {
      return emptyUploadState();
    }
    const parsed = JSON.parse(raw) as Partial<PersistedUploadState>;
    const totalUploadedCount =
      typeof parsed.totalUploadedCount === "number" && parsed.totalUploadedCount >= 0
        ? Math.floor(parsed.totalUploadedCount)
        : 0;
    const tasks = Array.isArray(parsed.tasks) ? parsed.tasks : [];
    return {
      totalUploadedCount,
      tasks,
      updatedAt: typeof parsed.updatedAt === "number" ? parsed.updatedAt : Date.now(),
      minimized: Boolean(parsed.minimized),
    };
  } catch {
    return emptyUploadState();
  }
}

export function saveUploadState(userId: number, state: PersistedUploadState): void {
  try {
    const trimmedTasks =
      state.tasks.length > MAX_STORED_TASKS
        ? state.tasks.slice(state.tasks.length - MAX_STORED_TASKS)
        : state.tasks;
    const payload: PersistedUploadState = {
      totalUploadedCount: state.totalUploadedCount,
      tasks: trimmedTasks,
      updatedAt: Date.now(),
      minimized: state.minimized,
    };
    localStorage.setItem(storageKey(userId), JSON.stringify(payload));
  } catch {
    // Ignora falhas de quota no localStorage sem interromper o fluxo de envio
  }
}

export function clearUploadState(userId: number): void {
  try {
    localStorage.removeItem(storageKey(userId));
  } catch {
    // ignore
  }
  void clearQueuedBlobsForUser(userId);
}

function openQueueDb(): Promise<IDBDatabase | null> {
  if (typeof indexedDB === "undefined") {
    return Promise.resolve(null);
  }
  return new Promise((resolve) => {
    try {
      const req = indexedDB.open(IDB_NAME, 1);
      req.onupgradeneeded = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains(IDB_STORE)) {
          db.createObjectStore(IDB_STORE, { keyPath: "key" });
        }
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => resolve(null);
    } catch {
      resolve(null);
    }
  });
}

function blobKey(userId: number, taskId: string): string {
  return `${userId}:${taskId}`;
}

export async function putQueuedFileBlob(
  userId: number,
  taskId: string,
  file: File,
): Promise<void> {
  const db = await openQueueDb();
  if (!db) {
    return;
  }
  await new Promise<void>((resolve) => {
    try {
      const tx = db.transaction(IDB_STORE, "readwrite");
      tx.objectStore(IDB_STORE).put({ key: blobKey(userId, taskId), userId, file });
      tx.oncomplete = () => {
        db.close();
        resolve();
      };
      tx.onerror = () => {
        db.close();
        resolve();
      };
    } catch {
      db.close();
      resolve();
    }
  });
}

export async function getQueuedFileBlob(
  userId: number,
  taskId: string,
): Promise<File | null> {
  const db = await openQueueDb();
  if (!db) {
    return null;
  }
  return new Promise<File | null>((resolve) => {
    try {
      const tx = db.transaction(IDB_STORE, "readonly");
      const req = tx.objectStore(IDB_STORE).get(blobKey(userId, taskId));
      req.onsuccess = () => {
        const record = req.result as { file?: File } | undefined;
        db.close();
        resolve(record?.file instanceof File ? record.file : null);
      };
      req.onerror = () => {
        db.close();
        resolve(null);
      };
    } catch {
      db.close();
      resolve(null);
    }
  });
}

export async function deleteQueuedFileBlob(userId: number, taskId: string): Promise<void> {
  const db = await openQueueDb();
  if (!db) {
    return;
  }
  await new Promise<void>((resolve) => {
    try {
      const tx = db.transaction(IDB_STORE, "readwrite");
      tx.objectStore(IDB_STORE).delete(blobKey(userId, taskId));
      tx.oncomplete = () => {
        db.close();
        resolve();
      };
      tx.onerror = () => {
        db.close();
        resolve();
      };
    } catch {
      db.close();
      resolve();
    }
  });
}

export async function clearQueuedBlobsForUser(userId: number): Promise<void> {
  const db = await openQueueDb();
  if (!db) {
    return;
  }
  await new Promise<void>((resolve) => {
    try {
      const tx = db.transaction(IDB_STORE, "readwrite");
      const store = tx.objectStore(IDB_STORE);
      const cursorReq = store.openCursor();
      const prefix = `${userId}:`;
      cursorReq.onsuccess = () => {
        const cursor = cursorReq.result;
        if (!cursor) {
          return;
        }
        if (typeof cursor.key === "string" && cursor.key.startsWith(prefix)) {
          cursor.delete();
        }
        cursor.continue();
      };
      tx.oncomplete = () => {
        db.close();
        resolve();
      };
      tx.onerror = () => {
        db.close();
        resolve();
      };
    } catch {
      db.close();
      resolve();
    }
  });
}
