import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type DragEvent,
  type FormEvent,
  type MouseEvent,
} from "react";
import { Navigate } from "react-router-dom";
import {
  ApiError,
  batchMoveFiles,
  createFolder,
  daysLeft,
  downloadFile,
  emptyTrash,
  folderPath,
  getStorageStats,
  listFiles,
  listFolders,
  listTrash,
  moveFile,
  moveFolder,
  purgeFile,
  purgeFolder,
  renameFile,
  renameFolder,
  restoreFile,
  restoreFolder,
  searchDrive,
  trashFile,
  trashFolder,
  uploadFile,
  type DriveFile,
  type DriveFolder,
  type SearchListing,
  type StorageStats,
} from "../api";
import { useAuth } from "../auth";
import { FileViewer } from "../components/FileViewer";
import { MediaThumb } from "../components/MediaThumb";
import {
  UploadConflictModal,
  type ConflictResolution,
} from "../components/UploadConflictModal";
import { UploadPanel } from "../components/UploadPanel";
import {
  FILTER_OPTIONS,
  cleanUploadFileName,
  fileCategory,
  formatBytes,
  formatStoragePair,
  matchesSearch,
  normalizeSearch,
  suggestFreeFileName,
  type EntryFilterKind,
} from "../media";
import {
  clearUploadState,
  deleteQueuedFileBlob,
  emptyUploadState,
  getQueuedFileBlob,
  loadUploadState,
  putQueuedFileBlob,
  saveUploadState,
  type PersistedUploadState,
  type UploadTaskRecord,
} from "../uploadPersistence";
import {
  ensureFolderPath,
  itemsFromDataTransfer,
  itemsFromFileList,
  type UploadItem,
} from "../uploadTree";

function ttlLabel(expiresAt?: string): string {
  const days = daysLeft(expiresAt);
  if (days === null) {
    return "Lixeira";
  }
  if (days <= 0) {
    return "Expira hoje";
  }
  if (days === 1) {
    return "1 dia restante";
  }
  return `${days} dias restantes`;
}

type ItemKind = "file" | "folder";
type SearchScope = "global" | "current";

type MovingTarget =
  | {
      mode: "single";
      kind: ItemKind;
      id: number;
      nome: string;
    }
  | {
      mode: "batch-files";
      ids: number[];
    };

type Renaming = {
  kind: ItemKind;
  id: number;
  nome: string;
};

type ConflictPromptState = {
  existingFile: DriveFile;
  incomingFile: File;
  suggestedName: string;
  existingNames: Set<string>;
  resolve: (resolution: ConflictResolution) => void;
};

function filterLocal(
  sourceFolders: DriveFolder[],
  sourceFiles: DriveFile[],
  rawQuery: string,
  kind: EntryFilterKind,
): SearchListing {
  const norm = normalizeSearch(rawQuery);
  const nextFolders =
    kind === "all" || kind === "folder"
      ? sourceFolders.filter((folder) => matchesSearch(folder.nome, norm))
      : [];
  const nextFiles =
    kind === "folder"
      ? []
      : sourceFiles.filter(
          (file) =>
            (kind === "all" || fileCategory(file) === kind) &&
            matchesSearch(file.nome, norm),
        );
  return { folders: nextFolders, files: nextFiles };
}

function makeTaskId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `up_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
}

export function DrivePage() {
  const { user, ready, logout } = useAuth();
  const [folderId, setFolderId] = useState<number | null>(null);
  const [crumbs, setCrumbs] = useState<DriveFolder[]>([]);
  const [folders, setFolders] = useState<DriveFolder[]>([]);
  const [files, setFiles] = useState<DriveFile[]>([]);
  const [inTrash, setInTrash] = useState(false);
  const [ttlDays, setTtlDays] = useState(30);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [drag, setDrag] = useState(false);
  const [naming, setNaming] = useState(false);
  const [newName, setNewName] = useState("");
  const [renaming, setRenaming] = useState<Renaming | null>(null);
  const [moving, setMoving] = useState<MovingTarget | null>(null);
  const [previewId, setPreviewId] = useState<number | null>(null);
  const [confirmEmptyTrash, setConfirmEmptyTrash] = useState(false);
  const [emptyingTrash, setEmptyingTrash] = useState(false);

  const [storageStats, setStorageStats] = useState<StorageStats | null>(null);
  const [fileVersions, setFileVersions] = useState<Record<number, number>>({});

  const [selectedFileIds, setSelectedFileIds] = useState<Set<number>>(() => new Set());
  const [lastSelectedFileId, setLastSelectedFileId] = useState<number | null>(null);

  const [uploadState, setUploadState] = useState<PersistedUploadState>(emptyUploadState);
  const [uploadingActive, setUploadingActive] = useState(false);
  const [conflictPrompt, setConflictPrompt] = useState<ConflictPromptState | null>(null);

  const [searchQuery, setSearchQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [typeFilter, setTypeFilter] = useState<EntryFilterKind>("all");
  const [searchScope, setSearchScope] = useState<SearchScope>("global");
  const [globalResults, setGlobalResults] = useState<SearchListing | null>(null);
  const [searching, setSearching] = useState(false);
  const [reloadTick, setReloadTick] = useState(0);

  const inputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const blobCache = useRef(new Map<number, string>());

  const uploadStateRef = useRef<PersistedUploadState>(emptyUploadState());
  const inMemoryFilesRef = useRef(new Map<string, File>());
  const workerRunningRef = useRef(false);
  const batchAbortRef = useRef<AbortController | null>(null);
  const conflictResolverRef = useRef<((res: ConflictResolution) => void) | null>(null);
  const folderIdRef = useRef<number | null>(null);
  const inTrashRef = useRef(false);
  const bootedUserIdRef = useRef<number | null>(null);

  folderIdRef.current = folderId;
  inTrashRef.current = inTrash;

  const commitUploadState = useCallback(
    (
      updater:
        | PersistedUploadState
        | ((prev: PersistedUploadState) => PersistedUploadState),
    ) => {
      const next =
        typeof updater === "function" ? updater(uploadStateRef.current) : updater;
      uploadStateRef.current = next;
      setUploadState(next);
      if (user) {
        saveUploadState(user.id, next);
      }
    },
    [user],
  );

  const refreshStorage = useCallback(async () => {
    try {
      const stats = await getStorageStats();
      setStorageStats(stats);
    } catch {
      // Mantém último valor conhecido silenciosamente caso falhe
    }
  }, []);

  const refresh = useCallback(async () => {
    setError(null);
    setReloadTick((n) => n + 1);
    void refreshStorage();
    try {
      if (inTrash) {
        const trash = await listTrash();
        setFolders(trash.folders);
        setFiles(trash.files);
        setTtlDays(trash.ttl_days);
        setCrumbs([]);
        return;
      }
      const [nextFolders, nextFiles, path] = await Promise.all([
        listFolders(folderId),
        listFiles(folderId),
        folderId === null ? Promise.resolve([]) : folderPath(folderId),
      ]);
      setFolders(nextFolders);
      setFiles(nextFiles);
      setCrumbs(path);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Falha ao listar o diretório.");
    }
  }, [folderId, inTrash, refreshStorage]);

  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;

  useEffect(() => {
    if (user) {
      void refresh();
    }
  }, [user, refresh]);

  const askConflictDecision = useCallback(
    (
      existingFile: DriveFile,
      incomingFile: File,
      suggestedName: string,
      existingNames: Set<string>,
    ): Promise<ConflictResolution> =>
      new Promise<ConflictResolution>((resolve) => {
        const finish = (res: ConflictResolution) => {
          conflictResolverRef.current = null;
          setConflictPrompt(null);
          resolve(res);
        };
        conflictResolverRef.current = finish;
        setConflictPrompt({
          existingFile,
          incomingFile,
          suggestedName,
          existingNames,
          resolve: finish,
        });
      }),
    [],
  );

  const bumpFileVersion = useCallback((id: number) => {
    const cached = blobCache.current.get(id);
    if (cached) {
      URL.revokeObjectURL(cached);
      blobCache.current.delete(id);
    }
    setFileVersions((prev) => ({ ...prev, [id]: (prev[id] ?? 0) + 1 }));
  }, []);

  const runUploadWorker = useCallback(
    async (userId: number) => {
      if (workerRunningRef.current) {
        return;
      }
      workerRunningRef.current = true;
      setUploadingActive(true);

      const abortController = new AbortController();
      batchAbortRef.current = abortController;

      const folderTreeCache = new Map<string, DriveFolder[]>();
      const filesByFolderCache = new Map<string, DriveFile[]>();

      async function getSiblings(targetFolderId: number | null): Promise<DriveFile[]> {
        const key = targetFolderId === null ? "root" : String(targetFolderId);
        let list = filesByFolderCache.get(key);
        if (!list) {
          list = await listFiles(targetFolderId);
          filesByFolderCache.set(key, list);
        }
        return list;
      }

      try {
        while (!abortController.signal.aborted) {
          const nextTask = uploadStateRef.current.tasks.find(
            (t) => t.status === "queued",
          );
          if (!nextTask) {
            break;
          }

          if (nextTask.isFolderPlaceholder) {
            try {
              await ensureFolderPath(
                nextTask.rootFolderId,
                nextTask.dirs,
                folderTreeCache,
              );
              commitUploadState((prev) => ({
                ...prev,
                totalUploadedCount: prev.totalUploadedCount + 1,
                tasks: prev.tasks.map((t) =>
                  t.id === nextTask.id
                    ? { ...t, status: "done", resolution: "created" }
                    : t,
                ),
              }));
              if (!inTrashRef.current) {
                void refreshRef.current();
              }
            } catch (err) {
              const msg = err instanceof ApiError ? err.message : "Falha ao criar pasta.";
              commitUploadState((prev) => ({
                ...prev,
                tasks: prev.tasks.map((t) =>
                  t.id === nextTask.id ? { ...t, status: "error", errorMessage: msg } : t,
                ),
              }));
            }
            continue;
          }

          const file =
            inMemoryFilesRef.current.get(nextTask.id) ??
            (await getQueuedFileBlob(userId, nextTask.id));

          if (!file) {
            commitUploadState((prev) => ({
              ...prev,
              tasks: prev.tasks.map((t) =>
                t.id === nextTask.id ? { ...t, status: "interrupted" } : t,
              ),
            }));
            continue;
          }

          let destFolderId: number | null = nextTask.rootFolderId;
          try {
            destFolderId = await ensureFolderPath(
              nextTask.rootFolderId,
              nextTask.dirs,
              folderTreeCache,
            );

            const siblings = await getSiblings(destFolderId);
            const cleanName = cleanUploadFileName(file.name);
            const existing = siblings.find((item) => item.nome === cleanName);

            let onConflict: "keep_both" | "reject" | "overwrite" = "reject";
            let targetFileName: string | undefined;
            let resolutionKind: "created" | "kept_both" | "overwritten" = "created";

            if (existing) {
              commitUploadState((prev) => ({
                ...prev,
                tasks: prev.tasks.map((t) =>
                  t.id === nextTask.id ? { ...t, status: "conflict" } : t,
                ),
              }));

              const existingNames = new Set(siblings.map((f) => f.nome));
              const suggested = suggestFreeFileName(cleanName, existingNames);
              const decision = await askConflictDecision(
                existing,
                file,
                suggested,
                existingNames,
              );

              if (abortController.signal.aborted || decision.action === "cancel") {
                inMemoryFilesRef.current.delete(nextTask.id);
                void deleteQueuedFileBlob(userId, nextTask.id);
                commitUploadState((prev) => ({
                  ...prev,
                  tasks: prev.tasks.map((t) =>
                    t.id === nextTask.id
                      ? { ...t, status: "skipped", loadedBytes: 0 }
                      : t,
                  ),
                }));
                continue;
              }

              if (decision.action === "overwrite") {
                onConflict = "overwrite";
                targetFileName = cleanName;
                resolutionKind = "overwritten";
              } else {
                onConflict = "keep_both";
                targetFileName = decision.newName;
                resolutionKind = "kept_both";
              }
            }

            commitUploadState((prev) => ({
              ...prev,
              tasks: prev.tasks.map((t) =>
                t.id === nextTask.id
                  ? { ...t, status: "uploading", loadedBytes: 0 }
                  : t,
              ),
            }));

            let lastProgressUiAt = 0;
            const onProgress = (loaded: number) => {
              const now = performance.now();
              if (loaded < file.size && now - lastProgressUiAt < 60) {
                return;
              }
              lastProgressUiAt = now;
              const nextState: PersistedUploadState = {
                ...uploadStateRef.current,
                tasks: uploadStateRef.current.tasks.map((t) =>
                  t.id === nextTask.id ? { ...t, loadedBytes: loaded } : t,
                ),
              };
              uploadStateRef.current = nextState;
              setUploadState(nextState);
            };

            let savedFile: DriveFile;
            try {
              savedFile = await uploadFile(file, destFolderId, {
                onConflict,
                fileName: targetFileName,
                onProgress,
                signal: abortController.signal,
              });
            } catch (uploadErr) {
              // Caso tenha surgido conflito simultâneo no servidor, pergunta ao usuário
              if (
                uploadErr instanceof ApiError &&
                uploadErr.status === 409 &&
                onConflict === "reject"
              ) {
                const key = destFolderId === null ? "root" : String(destFolderId);
                const freshSiblings = await listFiles(destFolderId);
                filesByFolderCache.set(key, freshSiblings);
                const freshExisting = freshSiblings.find(
                  (item) => item.nome === cleanName,
                );
                if (freshExisting) {
                  commitUploadState((prev) => ({
                    ...prev,
                    tasks: prev.tasks.map((t) =>
                      t.id === nextTask.id
                        ? { ...t, status: "conflict", loadedBytes: 0 }
                        : t,
                    ),
                  }));
                  const existingNames = new Set(freshSiblings.map((f) => f.nome));
                  const suggested = suggestFreeFileName(cleanName, existingNames);
                  const decision = await askConflictDecision(
                    freshExisting,
                    file,
                    suggested,
                    existingNames,
                  );
                  if (abortController.signal.aborted || decision.action === "cancel") {
                    inMemoryFilesRef.current.delete(nextTask.id);
                    void deleteQueuedFileBlob(userId, nextTask.id);
                    commitUploadState((prev) => ({
                      ...prev,
                      tasks: prev.tasks.map((t) =>
                        t.id === nextTask.id
                          ? { ...t, status: "skipped", loadedBytes: 0 }
                          : t,
                      ),
                    }));
                    continue;
                  }
                  commitUploadState((prev) => ({
                    ...prev,
                    tasks: prev.tasks.map((t) =>
                      t.id === nextTask.id
                        ? { ...t, status: "uploading", loadedBytes: 0 }
                        : t,
                    ),
                  }));
                  if (decision.action === "overwrite") {
                    resolutionKind = "overwritten";
                    savedFile = await uploadFile(file, destFolderId, {
                      onConflict: "overwrite",
                      fileName: cleanName,
                      onProgress,
                      signal: abortController.signal,
                    });
                  } else {
                    resolutionKind = "kept_both";
                    savedFile = await uploadFile(file, destFolderId, {
                      onConflict: "keep_both",
                      fileName: decision.newName,
                      onProgress,
                      signal: abortController.signal,
                    });
                  }
                } else {
                  resolutionKind = "kept_both";
                  savedFile = await uploadFile(file, destFolderId, {
                    onConflict: "keep_both",
                    onProgress,
                    signal: abortController.signal,
                  });
                }
              } else {
                throw uploadErr;
              }
            }

            if (resolutionKind === "overwritten") {
              bumpFileVersion(savedFile.id);
              const idx = siblings.findIndex((f) => f.id === savedFile.id);
              if (idx >= 0) {
                siblings[idx] = savedFile;
              } else {
                siblings.push(savedFile);
              }
            } else {
              siblings.push(savedFile);
            }

            inMemoryFilesRef.current.delete(nextTask.id);
            void deleteQueuedFileBlob(userId, nextTask.id);

            commitUploadState((prev) => ({
              ...prev,
              totalUploadedCount: prev.totalUploadedCount + 1,
              tasks: prev.tasks.map((t) =>
                t.id === nextTask.id
                  ? {
                      ...t,
                      status: "done",
                      finalName: savedFile.nome,
                      loadedBytes: file.size,
                      resolution: resolutionKind,
                    }
                  : t,
              ),
            }));

            if (!inTrashRef.current && destFolderId === folderIdRef.current) {
              void refreshRef.current();
            } else {
              void refreshStorage();
            }
          } catch (err) {
            if (abortController.signal.aborted || (err instanceof DOMException && err.name === "AbortError")) {
              break;
            }
            const msg = err instanceof ApiError ? err.message : "Falha no envio.";
            inMemoryFilesRef.current.delete(nextTask.id);
            void deleteQueuedFileBlob(userId, nextTask.id);
            commitUploadState((prev) => ({
              ...prev,
              tasks: prev.tasks.map((t) =>
                t.id === nextTask.id
                  ? { ...t, status: "error", errorMessage: msg }
                  : t,
              ),
            }));
            setError(msg);
          }
        }
      } finally {
        workerRunningRef.current = false;
        batchAbortRef.current = null;
        setUploadingActive(false);
        void refreshRef.current();
      }
    },
    [askConflictDecision, bumpFileVersion, commitUploadState, refreshStorage],
  );

  // Restaura contagem de itens upados (e retoma envios pendentes salvos em IndexedDB) ao carregar/voltar à página
  useEffect(() => {
    if (!user) {
      bootedUserIdRef.current = null;
      return;
    }
    if (bootedUserIdRef.current === user.id) {
      return;
    }
    bootedUserIdRef.current = user.id;

    const restored = loadUploadState(user.id);
    let hasPendingToResume = false;
    const normalizedTasks: UploadTaskRecord[] = restored.tasks.map((t) => {
      if (t.status === "uploading" || t.status === "conflict" || t.status === "queued") {
        hasPendingToResume = true;
        return { ...t, status: "queued", loadedBytes: 0 };
      }
      return t;
    });

    const initial: PersistedUploadState = {
      ...restored,
      tasks: normalizedTasks,
    };
    uploadStateRef.current = initial;
    setUploadState(initial);
    saveUploadState(user.id, initial);

    if (hasPendingToResume) {
      void runUploadWorker(user.id);
    }
  }, [user, runUploadWorker]);

  useEffect(() => {
    if (!uploadingActive) {
      return;
    }
    function onBeforeUnload(event: BeforeUnloadEvent) {
      if (user) {
        saveUploadState(user.id, uploadStateRef.current);
      }
      event.preventDefault();
    }
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, [uploadingActive, user]);

  useEffect(() => {
    const trimmed = searchQuery.trim();
    if (!trimmed) {
      setDebouncedQuery("");
      return;
    }
    const timer = window.setTimeout(() => {
      setDebouncedQuery(trimmed);
    }, 180);
    return () => window.clearTimeout(timer);
  }, [searchQuery]);

  useEffect(() => {
    const active =
      !inTrash &&
      searchScope === "global" &&
      (debouncedQuery.length > 0 || typeFilter !== "all");
    if (!user || !active) {
      setGlobalResults(null);
      setSearching(false);
      return;
    }
    const controller = new AbortController();
    setSearching(true);
    void searchDrive({ q: debouncedQuery, kind: typeFilter }, controller.signal)
      .then((res) => {
        if (!controller.signal.aborted) {
          setGlobalResults(res);
          setSearching(false);
        }
      })
      .catch((err: unknown) => {
        if (controller.signal.aborted) {
          return;
        }
        setSearching(false);
        setError(err instanceof ApiError ? err.message : "Falha ao pesquisar no Drive.");
      });
    return () => {
      controller.abort();
    };
  }, [user, inTrash, searchScope, debouncedQuery, typeFilter, reloadTick]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (
        previewId !== null ||
        moving !== null ||
        naming ||
        renaming !== null ||
        conflictPrompt !== null ||
        confirmEmptyTrash
      ) {
        return;
      }
      if (event.key === "Escape" && selectedFileIds.size > 0) {
        event.preventDefault();
        setSelectedFileIds(new Set());
        setLastSelectedFileId(null);
        return;
      }
      if (event.key !== "/" || event.ctrlKey || event.metaKey || event.altKey) {
        return;
      }
      const el = document.activeElement;
      if (
        el instanceof HTMLInputElement ||
        el instanceof HTMLTextAreaElement ||
        el instanceof HTMLSelectElement ||
        (el instanceof HTMLElement && el.isContentEditable)
      ) {
        return;
      }
      event.preventDefault();
      searchInputRef.current?.focus();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [
    previewId,
    moving,
    naming,
    renaming,
    conflictPrompt,
    confirmEmptyTrash,
    selectedFileIds.size,
  ]);

  useEffect(() => {
    const cache = blobCache.current;
    return () => {
      cache.forEach((url) => URL.revokeObjectURL(url));
      cache.clear();
    };
  }, []);

  if (ready && !user) {
    return <Navigate to="/login" replace />;
  }

  function clearSelection() {
    setSelectedFileIds(new Set());
    setLastSelectedFileId(null);
  }

  function clearSearchText() {
    setSearchQuery("");
    setDebouncedQuery("");
  }

  function clearFilters() {
    setSearchQuery("");
    setDebouncedQuery("");
    setTypeFilter("all");
    setGlobalResults(null);
    setSearching(false);
  }

  async function sendItems(items: UploadItem[]) {
    if (!user || items.length === 0) {
      return;
    }
    setError(null);
    setNotice(null);

    const targetRoot = folderId;
    const newRecords: UploadTaskRecord[] = [];

    for (const item of items) {
      const id = makeTaskId();
      if (item.file) {
        inMemoryFilesRef.current.set(id, item.file);
        void putQueuedFileBlob(user.id, id, item.file);
        newRecords.push({
          id,
          name: cleanUploadFileName(item.file.name),
          size: item.file.size,
          loadedBytes: 0,
          dirs: item.dirs,
          rootFolderId: targetRoot,
          status: "queued",
        });
      } else {
        newRecords.push({
          id,
          name: item.dirs.at(-1) ?? "Pasta",
          size: 0,
          loadedBytes: 0,
          dirs: item.dirs,
          rootFolderId: targetRoot,
          isFolderPlaceholder: true,
          status: "queued",
        });
      }
    }

    commitUploadState((prev) => ({
      ...prev,
      minimized: false,
      tasks: [...prev.tasks, ...newRecords],
    }));

    void runUploadWorker(user.id);
  }

  function cancelActiveUploads() {
    if (!user) {
      return;
    }
    conflictResolverRef.current?.({ action: "cancel" });
    batchAbortRef.current?.abort();

    for (const task of uploadStateRef.current.tasks) {
      if (
        task.status === "queued" ||
        task.status === "uploading" ||
        task.status === "conflict"
      ) {
        inMemoryFilesRef.current.delete(task.id);
        void deleteQueuedFileBlob(user.id, task.id);
      }
    }

    commitUploadState((prev) => ({
      ...prev,
      tasks: prev.tasks.map((t) =>
        t.status === "queued" || t.status === "uploading" || t.status === "conflict"
          ? { ...t, status: "skipped", loadedBytes: 0 }
          : t,
      ),
    }));
  }

  function clearUploadHistory() {
    if (!user || uploadingActive) {
      return;
    }
    inMemoryFilesRef.current.clear();
    clearUploadState(user.id);
    const fresh = emptyUploadState();
    uploadStateRef.current = fresh;
    setUploadState(fresh);
  }

  function toggleMinimizeUploadPanel() {
    commitUploadState((prev) => ({ ...prev, minimized: !prev.minimized }));
  }

  function onDragOver(event: DragEvent<HTMLElement>) {
    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
    if (!inTrash) {
      setDrag(true);
    }
  }

  function onDragLeave(event: DragEvent<HTMLElement>) {
    if (!event.currentTarget.contains(event.relatedTarget as Node)) {
      setDrag(false);
    }
  }

  function onDrop(event: DragEvent<HTMLElement>) {
    event.preventDefault();
    setDrag(false);
    if (inTrash) {
      return;
    }
    void itemsFromDataTransfer(event.dataTransfer).then((items) => sendItems(items));
  }

  async function onDownload(file: DriveFile) {
    setError(null);
    try {
      await downloadFile(file.id, file.nome);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Falha no download.");
    }
  }

  function openPreview(file: DriveFile) {
    setError(null);
    setPreviewId(file.id);
  }

  function goTo(id: number | null) {
    clearSearchText();
    clearSelection();
    setNotice(null);
    if (searchScope === "global" || typeFilter === "folder") {
      setTypeFilter("all");
    }
    setInTrash(false);
    setRenaming(null);
    setPreviewId(null);
    if (id === null) {
      setCrumbs([]);
      setFolderId(null);
      return;
    }
    setCrumbs((prev) => {
      const index = prev.findIndex((crumb) => crumb.id === id);
      return index >= 0 ? prev.slice(0, index + 1) : prev;
    });
    setFolderId(id);
  }

  function openFolder(folder: DriveFolder) {
    if (inTrash) {
      return;
    }
    clearSearchText();
    clearSelection();
    setNotice(null);
    if (searchScope === "global" || typeFilter === "folder") {
      setTypeFilter("all");
    }
    setPreviewId(null);
    setCrumbs((prev) => [...prev, folder]);
    setFolderId(folder.id);
  }

  function openContainingFolder(targetFolderId: number | null) {
    clearFilters();
    clearSelection();
    setNotice(null);
    setInTrash(false);
    setRenaming(null);
    setPreviewId(null);
    if (targetFolderId === null) {
      setCrumbs([]);
      setFolderId(null);
      return;
    }
    setFolderId(targetFolderId);
  }

  function openTrash() {
    clearFilters();
    clearSelection();
    setNotice(null);
    setFolders([]);
    setFiles([]);
    setInTrash(true);
    setNaming(false);
    setRenaming(null);
    setMoving(null);
    setPreviewId(null);
  }

  async function onCreateFolder(event: FormEvent) {
    event.preventDefault();
    setError(null);
    try {
      await createFolder(newName, folderId);
      setNewName("");
      setNaming(false);
      await refresh();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Não foi possível criar a pasta.");
    }
  }

  async function onRename(event: FormEvent) {
    event.preventDefault();
    if (!renaming) {
      return;
    }
    setError(null);
    try {
      if (renaming.kind === "folder") {
        await renameFolder(renaming.id, renaming.nome);
      } else {
        await renameFile(renaming.id, renaming.nome);
      }
      setRenaming(null);
      await refresh();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Não foi possível renomear.");
    }
  }

  async function onMove(dest: number | null) {
    if (!moving) {
      return;
    }
    setError(null);
    setNotice(null);
    try {
      if (moving.mode === "single") {
        if (moving.kind === "folder") {
          await moveFolder(moving.id, dest);
        } else {
          await moveFile(moving.id, dest);
          setSelectedFileIds((prev) => {
            if (!prev.has(moving.id)) {
              return prev;
            }
            const next = new Set(prev);
            next.delete(moving.id);
            return next;
          });
        }
        setMoving(null);
        await refresh();
      } else {
        const res = await batchMoveFiles(moving.ids, dest);
        setMoving(null);
        if (res.conflict_ids.length > 0) {
          setSelectedFileIds(new Set(res.conflict_ids));
          const names = res.conflict_names.slice(0, 3).join(", ");
          const extra =
            res.conflict_names.length > 3
              ? ` e mais ${res.conflict_names.length - 3}`
              : "";
          setError(
            `${res.moved_ids.length} movido(s). Conflito de nome no destino para: ${names}${extra}.`,
          );
        } else {
          clearSelection();
          setNotice(
            res.moved_ids.length === 1
              ? "1 arquivo movido."
              : `${res.moved_ids.length} arquivos movidos.`,
          );
        }
        await refresh();
      }
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Não foi possível mover.");
    }
  }

  async function onTrash(kind: ItemKind, id: number) {
    setError(null);
    try {
      if (kind === "folder") {
        await trashFolder(id);
      } else {
        await trashFile(id);
        setSelectedFileIds((prev) => {
          if (!prev.has(id)) {
            return prev;
          }
          const next = new Set(prev);
          next.delete(id);
          return next;
        });
      }
      await refresh();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Não foi possível enviar à lixeira.");
    }
  }

  async function onRestore(kind: ItemKind, id: number) {
    setError(null);
    try {
      if (kind === "folder") {
        await restoreFolder(id);
      } else {
        await restoreFile(id);
      }
      await refresh();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Não foi possível restaurar.");
    }
  }

  async function onPurge(kind: ItemKind, id: number, nome: string) {
    if (!window.confirm(`Excluir "${nome}" de vez? Isso não tem volta.`)) {
      return;
    }
    setError(null);
    try {
      if (kind === "folder") {
        await purgeFolder(id);
      } else {
        await purgeFile(id);
      }
      await refresh();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Não foi possível excluir de vez.");
    }
  }

  async function onConfirmEmptyTrash() {
    setError(null);
    setEmptyingTrash(true);
    try {
      await emptyTrash();
      setConfirmEmptyTrash(false);
      await refresh();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Não foi possível limpar a lixeira.");
    } finally {
      setEmptyingTrash(false);
    }
  }

  if (!ready || !user) {
    return (
      <main className="drive-shell">
        <p className="muted">Abrindo drive…</p>
      </main>
    );
  }

  const trimmedQuery = searchQuery.trim();
  const hasActiveFilter = trimmedQuery.length > 0 || typeFilter !== "all";
  const useGlobalSearch = !inTrash && searchScope === "global" && hasActiveFilter;
  const isPendingGlobal = useGlobalSearch && (searching || trimmedQuery !== debouncedQuery);

  const visibleListing = useGlobalSearch
    ? globalResults
      ? filterLocal(globalResults.folders, globalResults.files, trimmedQuery, typeFilter)
      : filterLocal(folders, files, trimmedQuery, typeFilter)
    : hasActiveFilter
      ? filterLocal(folders, files, trimmedQuery, typeFilter)
      : { folders, files };

  const visibleFolders = visibleListing.folders;
  const visibleFiles = visibleListing.files;
  const empty = visibleFolders.length === 0 && visibleFiles.length === 0;
  const totalResults = visibleFolders.length + visibleFiles.length;
  const trashTotalCount = folders.length + files.length;

  function toggleFileSelection(fileId: number, shiftKey: boolean) {
    setNotice(null);
    setSelectedFileIds((prev) => {
      const next = new Set(prev);
      if (shiftKey && lastSelectedFileId !== null && visibleFiles.length > 1) {
        const idxA = visibleFiles.findIndex((f) => f.id === lastSelectedFileId);
        const idxB = visibleFiles.findIndex((f) => f.id === fileId);
        if (idxA >= 0 && idxB >= 0) {
          const [from, to] = idxA < idxB ? [idxA, idxB] : [idxB, idxA];
          const shouldSelect = !prev.has(fileId);
          for (let i = from; i <= to; i += 1) {
            if (shouldSelect) {
              next.add(visibleFiles[i].id);
            } else {
              next.delete(visibleFiles[i].id);
            }
          }
          return next;
        }
      }
      if (next.has(fileId)) {
        next.delete(fileId);
      } else {
        next.add(fileId);
      }
      return next;
    });
    setLastSelectedFileId(fileId);
  }

  const allVisibleFilesSelected =
    visibleFiles.length > 0 && visibleFiles.every((f) => selectedFileIds.has(f.id));

  function toggleSelectAllVisible() {
    setNotice(null);
    if (allVisibleFilesSelected) {
      setSelectedFileIds((prev) => {
        const next = new Set(prev);
        for (const f of visibleFiles) {
          next.delete(f.id);
        }
        return next;
      });
    } else {
      setSelectedFileIds((prev) => {
        const next = new Set(prev);
        for (const f of visibleFiles) {
          next.add(f.id);
        }
        return next;
      });
    }
  }

  function handleFileTileClick(event: MouseEvent, file: DriveFile) {
    if (!inTrash && (selectedFileIds.size > 0 || event.shiftKey)) {
      event.preventDefault();
      toggleFileSelection(file.id, event.shiftKey);
      return;
    }
    openPreview(file);
  }

  const storagePct =
    storageStats && storageStats.total_bytes > 0
      ? Math.min(100, Math.max(0, (storageStats.used_bytes / storageStats.total_bytes) * 100))
      : 0;

  const title = inTrash
    ? "Lixeira"
    : folderId === null
      ? `Drive de ${user.name}`
      : (crumbs.at(-1)?.nome ?? "Pasta");

  const searchPlaceholder = inTrash
    ? "Pesquisar na lixeira…"
    : searchScope === "global"
      ? "Pesquisar arquivos e pastas em todo o Drive…"
      : "Pesquisar nesta pasta…";

  const scopeSummaryLabel = inTrash
    ? "na lixeira"
    : searchScope === "global"
      ? "em todo o Drive"
      : "nesta pasta";

  return (
    <main
      className={drag && !inTrash ? "drive-shell wide dropping" : "drive-shell wide"}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
    >
      <header className="topbar">
        <div className="topbar-main">
          <p className="eyebrow">Drive local</p>
          <h1>{title}</h1>
          {inTrash ? (
            <p className="hint">Itens somem após {ttlDays} dias.</p>
          ) : null}
        </div>

        <div className="topbar-right">
          {storageStats ? (
            <div
              className="storage-meter"
              title={`Uso de armazenamento da máquina: ${formatStoragePair(storageStats.used_bytes, storageStats.total_bytes)} (${Math.round(storagePct)}%)`}
            >
              <div className="storage-meter-top">
                <span className="storage-label">
                  <DiskMark />
                  Disco
                </span>
                <strong className="storage-numbers">
                  {formatStoragePair(storageStats.used_bytes, storageStats.total_bytes)}
                </strong>
              </div>
              <div className="storage-track" aria-hidden="true">
                <div
                  className={storagePct >= 90 ? "storage-fill high" : "storage-fill"}
                  style={{ width: `${storagePct}%` }}
                />
              </div>
            </div>
          ) : null}

          <div className="top-actions">
            {inTrash ? (
              <button
                type="button"
                className="danger"
                disabled={trashTotalCount === 0 || emptyingTrash}
                onClick={() => setConfirmEmptyTrash(true)}
              >
                Limpar lixeira
              </button>
            ) : null}
            <button
              type="button"
              className={inTrash ? "ghost current-view" : "ghost"}
              onClick={openTrash}
            >
              Lixeira
            </button>
            <button type="button" className="ghost" onClick={() => void logout()}>
              Sair
            </button>
          </div>
        </div>
      </header>

      <nav className="crumbs" aria-label="Caminho">
        <button
          type="button"
          className={!inTrash && folderId === null && !useGlobalSearch ? "crumb current" : "crumb"}
          onClick={() => goTo(null)}
        >
          Início
        </button>
        {!inTrash ? (
          crumbs.map((crumb) => (
            <span key={crumb.id} className="crumb-wrap">
              <span className="sep" aria-hidden="true">
                /
              </span>
              <button
                type="button"
                className={crumb.id === folderId && !useGlobalSearch ? "crumb current" : "crumb"}
                onClick={() => goTo(crumb.id)}
              >
                {crumb.nome}
              </button>
            </span>
          ))
        ) : (
          <span className="crumb-wrap">
            <span className="sep" aria-hidden="true">
              /
            </span>
            <span className="crumb current">Lixeira</span>
          </span>
        )}
      </nav>

      {!inTrash ? (
        <section className={drag ? "dropzone drag" : "dropzone"}>
          <p>
            {uploadingActive
              ? "Envio em andamento — você pode adicionar mais itens à fila"
              : "Solte arquivos ou pastas nesta pasta, ou"}
          </p>
          <div className="drop-actions">
            <button type="button" className="ghost" onClick={() => setNaming(true)}>
              Nova pasta
            </button>
            <button type="button" onClick={() => inputRef.current?.click()}>
              Enviar arquivos
            </button>
            <button
              type="button"
              className="ghost"
              onClick={() => folderInputRef.current?.click()}
            >
              Enviar pasta
            </button>
          </div>
          <input
            ref={inputRef}
            type="file"
            multiple
            hidden
            onChange={(e) => {
              if (e.target.files) {
                void sendItems(itemsFromFileList(e.target.files));
                e.target.value = "";
              }
            }}
          />
          <input
            ref={(el) => {
              folderInputRef.current = el;
              el?.setAttribute("webkitdirectory", "");
              el?.setAttribute("directory", "");
            }}
            type="file"
            multiple
            hidden
            onChange={(e) => {
              if (e.target.files) {
                void sendItems(itemsFromFileList(e.target.files));
                e.target.value = "";
              }
            }}
          />
        </section>
      ) : null}

      <UploadPanel
        state={uploadState}
        active={uploadingActive}
        onToggleMinimize={toggleMinimizeUploadPanel}
        onCancelActive={cancelActiveUploads}
        onClearHistory={clearUploadHistory}
      />

      {naming && !inTrash ? (
        <form className="new-folder" onSubmit={(e) => void onCreateFolder(e)}>
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="Nome da pasta"
            maxLength={80}
            autoFocus
            required
          />
          <button type="submit">Criar</button>
          <button
            type="button"
            className="ghost"
            onClick={() => {
              setNaming(false);
              setNewName("");
            }}
          >
            Cancelar
          </button>
        </form>
      ) : null}

      {renaming ? (
        <form className="new-folder" onSubmit={(e) => void onRename(e)}>
          <input
            type="text"
            value={renaming.nome}
            onChange={(e) => setRenaming({ ...renaming, nome: e.target.value })}
            placeholder="Novo nome"
            maxLength={80}
            autoFocus
            required
          />
          <button type="submit">Salvar</button>
          <button type="button" className="ghost" onClick={() => setRenaming(null)}>
            Cancelar
          </button>
        </form>
      ) : null}

      <section className="filter-bar" aria-label="Pesquisa e filtros">
        <div className="search-row">
          <div className="search-field">
            <SearchMark />
            <input
              ref={searchInputRef}
              type="search"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  e.preventDefault();
                  if (searchQuery) {
                    clearSearchText();
                  } else {
                    e.currentTarget.blur();
                  }
                }
              }}
              placeholder={searchPlaceholder}
              aria-label="Pesquisar por nome"
              autoComplete="off"
              spellCheck={false}
            />
            {searchQuery ? (
              <button
                type="button"
                className="search-clear"
                onClick={() => {
                  clearSearchText();
                  searchInputRef.current?.focus();
                }}
                aria-label="Limpar pesquisa"
                title="Limpar pesquisa"
              >
                ×
              </button>
            ) : (
              <kbd className="search-key" aria-hidden="true">
                /
              </kbd>
            )}
          </div>

          {!inTrash ? (
            <div className="scope-switch" role="group" aria-label="Escopo da busca">
              <button
                type="button"
                className={searchScope === "global" ? "scope-btn active" : "scope-btn"}
                aria-pressed={searchScope === "global"}
                onClick={() => setSearchScope("global")}
              >
                Todo o Drive
              </button>
              <button
                type="button"
                className={searchScope === "current" ? "scope-btn active" : "scope-btn"}
                aria-pressed={searchScope === "current"}
                onClick={() => setSearchScope("current")}
              >
                Pasta atual
              </button>
            </div>
          ) : null}
        </div>

        <div className="filter-chips-row">
          <div className="type-chips" role="group" aria-label="Filtrar por tipo de arquivo">
            {FILTER_OPTIONS.map((opt) => {
              const active = typeFilter === opt.value;
              return (
                <button
                  key={opt.value}
                  type="button"
                  className={active ? "type-chip active" : "type-chip"}
                  aria-pressed={active}
                  onClick={() => {
                    if (opt.value !== typeFilter) {
                      setGlobalResults(null);
                      setTypeFilter(opt.value);
                    } else if (opt.value !== "all") {
                      setGlobalResults(null);
                      setTypeFilter("all");
                    }
                  }}
                >
                  {opt.label}
                </button>
              );
            })}
          </div>

          <div className="filter-right-tools">
            {!inTrash && visibleFiles.length > 0 && selectedFileIds.size === 0 ? (
              <button
                type="button"
                className="filter-reset"
                onClick={toggleSelectAllVisible}
              >
                Selecionar arquivos
              </button>
            ) : null}

            {hasActiveFilter ? (
              <div className="filter-status">
                <span>
                  {isPendingGlobal
                    ? "Buscando…"
                    : `${totalResults} ${totalResults === 1 ? "item" : "itens"} ${scopeSummaryLabel}`}
                </span>
                <button type="button" className="filter-reset" onClick={clearFilters}>
                  Limpar
                </button>
              </div>
            ) : null}
          </div>
        </div>
      </section>

      {!inTrash && selectedFileIds.size > 0 ? (
        <section className="selection-bar" aria-label="Ações de seleção em lote">
          <div className="selection-info">
            <strong>
              {selectedFileIds.size}{" "}
              {selectedFileIds.size === 1 ? "arquivo selecionado" : "arquivos selecionados"}
            </strong>
            {visibleFiles.length > 0 ? (
              <button
                type="button"
                className="ghost selection-link"
                onClick={toggleSelectAllVisible}
              >
                {allVisibleFilesSelected ? "Desmarcar visíveis" : "Selecionar todos"}
              </button>
            ) : null}
          </div>
          <div className="selection-actions">
            <button
              type="button"
              onClick={() =>
                setMoving({
                  mode: "batch-files",
                  ids: Array.from(selectedFileIds),
                })
              }
            >
              Mover selecionados
            </button>
            <button type="button" className="ghost" onClick={clearSelection}>
              Cancelar
            </button>
          </div>
        </section>
      ) : null}

      {error ? <p className="error">{error}</p> : null}
      {notice ? <p className="notice">{notice}</p> : null}

      {empty ? (
        isPendingGlobal ? (
          <p className="empty">Buscando em todo o Drive…</p>
        ) : hasActiveFilter ? (
          <div className="empty empty-filtered">
            <p>Nenhum item encontrado {scopeSummaryLabel} para este filtro.</p>
            <button type="button" className="ghost" onClick={clearFilters}>
              Limpar pesquisa e filtros
            </button>
          </div>
        ) : (
          <p className="empty">
            {inTrash ? "Lixeira vazia." : "Pasta vazia. Crie um diretório ou envie um arquivo."}
          </p>
        )
      ) : (
        <ul className="drive-grid">
          {visibleFolders.map((folder) => {
            const folderSubtitle = inTrash
              ? ttlLabel(folder.expires_at)
              : folder.location
                ? `Pasta · ${folder.location}`
                : "Pasta";
            return (
              <li key={`d-${folder.id}`}>
                <div className="tile">
                  <button
                    type="button"
                    className="tile-hit"
                    onClick={() => openFolder(folder)}
                    disabled={inTrash}
                  >
                    <div className="tile-thumb kind-folder">
                      <FolderMark />
                    </div>
                    <div className="tile-meta">
                      <strong title={folder.nome}>{folder.nome}</strong>
                      <span title={folderSubtitle}>{folderSubtitle}</span>
                    </div>
                  </button>
                  <div className="tile-actions">
                    {inTrash ? (
                      <>
                        <button
                          type="button"
                          className="ghost"
                          onClick={() => void onRestore("folder", folder.id)}
                        >
                          Restaurar
                        </button>
                        <button
                          type="button"
                          className="danger"
                          onClick={() => void onPurge("folder", folder.id, folder.nome)}
                        >
                          Excluir de vez
                        </button>
                      </>
                    ) : (
                      <>
                        <button
                          type="button"
                          className="ghost"
                          onClick={() =>
                            setRenaming({ kind: "folder", id: folder.id, nome: folder.nome })
                          }
                        >
                          Renomear
                        </button>
                        <button
                          type="button"
                          className="ghost"
                          onClick={() =>
                            setMoving({
                              mode: "single",
                              kind: "folder",
                              id: folder.id,
                              nome: folder.nome,
                            })
                          }
                        >
                          Mover
                        </button>
                        <button
                          type="button"
                          className="danger"
                          onClick={() => void onTrash("folder", folder.id)}
                        >
                          Lixeira
                        </button>
                      </>
                    )}
                  </div>
                </div>
              </li>
            );
          })}
          {visibleFiles.map((file) => {
            const isSelected = !inTrash && selectedFileIds.has(file.id);
            const fileSubtitle = inTrash
              ? `${formatBytes(file.tamanho_bytes)} · ${ttlLabel(file.expires_at)}`
              : file.location
                ? `${formatBytes(file.tamanho_bytes)} · ${file.location}`
                : formatBytes(file.tamanho_bytes);
            const vTag = file.tamanho_bytes + (fileVersions[file.id] ?? 0);
            return (
              <li key={`f-${file.id}`}>
                <div className={isSelected ? "tile selected" : "tile"}>
                  {!inTrash ? (
                    <button
                      type="button"
                      className={isSelected ? "tile-select-btn checked" : "tile-select-btn"}
                      role="checkbox"
                      aria-checked={isSelected}
                      aria-label={`Selecionar ${file.nome}`}
                      title="Selecionar para mover em lote (Shift+clique para intervalo)"
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleFileSelection(file.id, e.shiftKey);
                      }}
                    >
                      {isSelected ? "✓" : ""}
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="tile-hit"
                    onClick={(e) => handleFileTileClick(e, file)}
                  >
                    <MediaThumb file={file} version={vTag} />
                    <div className="tile-meta">
                      <strong title={file.nome}>{file.nome}</strong>
                      <span title={fileSubtitle}>{fileSubtitle}</span>
                    </div>
                  </button>
                  <div className="tile-actions">
                    {inTrash ? (
                      <>
                        <button
                          type="button"
                          className="ghost"
                          onClick={() => void onRestore("file", file.id)}
                        >
                          Restaurar
                        </button>
                        <button
                          type="button"
                          className="danger"
                          onClick={() => void onPurge("file", file.id, file.nome)}
                        >
                          Excluir de vez
                        </button>
                      </>
                    ) : (
                      <>
                        <button
                          type="button"
                          className="ghost"
                          onClick={() => void onDownload(file)}
                        >
                          Baixar
                        </button>
                        {file.location ? (
                          <button
                            type="button"
                            className="ghost"
                            onClick={() => openContainingFolder(file.folder_id)}
                            title={`Abrir pasta: ${file.location}`}
                          >
                            Pasta
                          </button>
                        ) : null}
                        <button
                          type="button"
                          className="ghost"
                          onClick={() =>
                            setRenaming({ kind: "file", id: file.id, nome: file.nome })
                          }
                        >
                          Renomear
                        </button>
                        <button
                          type="button"
                          className="ghost"
                          onClick={() =>
                            setMoving({
                              mode: "single",
                              kind: "file",
                              id: file.id,
                              nome: file.nome,
                            })
                          }
                        >
                          Mover
                        </button>
                        <button
                          type="button"
                          className="danger"
                          onClick={() => void onTrash("file", file.id)}
                        >
                          Lixeira
                        </button>
                      </>
                    )}
                  </div>
                </div>
              </li>
            );
          })}
        </ul>
      )}

      {moving ? (
        <MovePicker
          target={moving}
          startId={folderId}
          startCrumbs={crumbs}
          onCancel={() => setMoving(null)}
          onConfirm={(dest) => void onMove(dest)}
        />
      ) : null}

      {conflictPrompt ? (
        <UploadConflictModal
          existingFile={conflictPrompt.existingFile}
          incomingFile={conflictPrompt.incomingFile}
          suggestedName={conflictPrompt.suggestedName}
          existingNames={conflictPrompt.existingNames}
          fileVersion={
            conflictPrompt.existingFile.tamanho_bytes +
            (fileVersions[conflictPrompt.existingFile.id] ?? 0)
          }
          onResolve={conflictPrompt.resolve}
        />
      ) : null}

      {confirmEmptyTrash ? (
        <div
          className="lightbox"
          onClick={() => !emptyingTrash && setConfirmEmptyTrash(false)}
          role="presentation"
        >
          <div
            className="lightbox-card confirm-card"
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-modal="true"
            aria-labelledby="empty-trash-title"
          >
            <header>
              <strong id="empty-trash-title">Limpar lixeira?</strong>
              <button
                type="button"
                className="ghost"
                disabled={emptyingTrash}
                onClick={() => setConfirmEmptyTrash(false)}
              >
                Fechar
              </button>
            </header>
            <p className="confirm-body">
              Excluir permanentemente todos os {trashTotalCount}{" "}
              {trashTotalCount === 1 ? "item" : "itens"} da lixeira? Essa operação libera espaço
              no disco imediatamente e não pode ser desfeita.
            </p>
            <div className="drop-actions">
              <button
                type="button"
                className="danger"
                disabled={emptyingTrash}
                onClick={() => void onConfirmEmptyTrash()}
              >
                {emptyingTrash ? "Limpando…" : "Excluir tudo de vez"}
              </button>
              <button
                type="button"
                className="ghost"
                disabled={emptyingTrash}
                onClick={() => setConfirmEmptyTrash(false)}
              >
                Cancelar
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {previewId !== null ? (
        <FileViewer
          files={visibleFiles}
          activeId={previewId}
          blobCache={blobCache.current}
          fileVersions={fileVersions}
          onSelectId={setPreviewId}
          onDownload={(file) => void onDownload(file)}
          onClose={() => setPreviewId(null)}
        />
      ) : null}
    </main>
  );
}

function MovePicker({
  target,
  startId,
  startCrumbs,
  onCancel,
  onConfirm,
}: {
  target: MovingTarget;
  startId: number | null;
  startCrumbs: DriveFolder[];
  onCancel: () => void;
  onConfirm: (dest: number | null) => void;
}) {
  const [here, setHere] = useState<number | null>(startId);
  const [path, setPath] = useState<DriveFolder[]>(startCrumbs);
  const [folders, setFolders] = useState<DriveFolder[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const next = await listFolders(here);
        if (!cancelled) {
          setFolders(
            next.filter(
              (folder) =>
                !(
                  target.mode === "single" &&
                  target.kind === "folder" &&
                  folder.id === target.id
                ),
            ),
          );
          setError(null);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof ApiError ? err.message : "Falha ao listar pastas.");
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [here, target]);

  function go(id: number | null) {
    if (id === null) {
      setPath([]);
      setHere(null);
      return;
    }
    setPath((prev) => {
      const index = prev.findIndex((crumb) => crumb.id === id);
      return index >= 0 ? prev.slice(0, index + 1) : prev;
    });
    setHere(id);
  }

  const heading =
    target.mode === "single"
      ? `Mover “${target.nome}”`
      : target.ids.length === 1
        ? "Mover 1 arquivo selecionado"
        : `Mover ${target.ids.length} arquivos selecionados`;

  return (
    <div className="lightbox" onClick={onCancel} role="presentation">
      <div
        className="lightbox-card move-card"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <header>
          <strong>{heading}</strong>
          <button type="button" className="ghost" onClick={onCancel}>
            Fechar
          </button>
        </header>
        <nav className="crumbs" aria-label="Destino">
          <button
            type="button"
            className={here === null ? "crumb current" : "crumb"}
            onClick={() => go(null)}
          >
            Início
          </button>
          {path.map((crumb) => (
            <span key={crumb.id} className="crumb-wrap">
              <span className="sep" aria-hidden="true">
                /
              </span>
              <button
                type="button"
                className={crumb.id === here ? "crumb current" : "crumb"}
                onClick={() => go(crumb.id)}
              >
                {crumb.nome}
              </button>
            </span>
          ))}
        </nav>
        {error ? <p className="error">{error}</p> : null}
        {folders.length === 0 ? (
          <p className="empty">Nenhuma subpasta aqui.</p>
        ) : (
          <ul className="move-list">
            {folders.map((folder) => (
              <li key={folder.id}>
                <button
                  type="button"
                  className="ghost"
                  onClick={() => {
                    setPath((prev) => [...prev, folder]);
                    setHere(folder.id);
                  }}
                >
                  {folder.nome}
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="drop-actions">
          <button type="button" onClick={() => onConfirm(here)}>
            Mover para cá
          </button>
          <button type="button" onClick={onCancel} className="ghost">
            Cancelar
          </button>
        </div>
      </div>
    </div>
  );
}

function DiskMark() {
  return (
    <svg viewBox="0 0 20 20" className="disk-icon" aria-hidden="true">
      <rect
        x="3"
        y="2.5"
        width="14"
        height="15"
        rx="2"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <circle cx="10" cy="8.5" r="3.6" fill="none" stroke="currentColor" strokeWidth="1.3" />
      <circle cx="10" cy="8.5" r="1" fill="currentColor" />
      <line x1="3.8" y1="14" x2="16.2" y2="14" stroke="currentColor" strokeWidth="1.2" />
      <circle cx="14" cy="15.8" r="0.9" fill="currentColor" />
    </svg>
  );
}

function SearchMark() {
  return (
    <svg viewBox="0 0 20 20" className="search-icon" aria-hidden="true">
      <circle cx="8.5" cy="8.5" r="5.5" fill="none" stroke="currentColor" strokeWidth="1.6" />
      <path
        d="M12.7 12.7L17 17"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
      />
    </svg>
  );
}

function FolderMark() {
  return (
    <svg viewBox="0 0 64 48" className="folder-svg" aria-hidden="true">
      <path
        d="M4 12c0-3 2-5 5-5h16l6 6h24c3 0 5 2 5 5v21c0 3-2 5-5 5H9c-3 0-5-2-5-5V12z"
        fill="currentColor"
        opacity="0.55"
      />
      <path d="M4 20h56v19c0 3-2 5-5 5H9c-3 0-5-2-5-5V20z" fill="currentColor" />
    </svg>
  );
}
