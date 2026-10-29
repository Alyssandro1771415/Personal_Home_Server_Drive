import { useCallback, useEffect, useRef, useState, type DragEvent, type FormEvent } from "react";
import { Navigate } from "react-router-dom";
import {
  ApiError,
  createFolder,
  daysLeft,
  downloadFile,
  folderPath,
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
} from "../api";
import { useAuth } from "../auth";
import { FileViewer } from "../components/FileViewer";
import { MediaThumb, loadBlobUrl } from "../components/MediaThumb";
import {
  FILTER_OPTIONS,
  fileCategory,
  formatBytes,
  matchesSearch,
  normalizeSearch,
  type EntryFilterKind,
} from "../media";
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

type Moving = {
  kind: ItemKind;
  id: number;
  nome: string;
};

type Renaming = {
  kind: ItemKind;
  id: number;
  nome: string;
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

export function DrivePage() {
  const { user, ready, logout } = useAuth();
  const [folderId, setFolderId] = useState<number | null>(null);
  const [crumbs, setCrumbs] = useState<DriveFolder[]>([]);
  const [folders, setFolders] = useState<DriveFolder[]>([]);
  const [files, setFiles] = useState<DriveFile[]>([]);
  const [inTrash, setInTrash] = useState(false);
  const [ttlDays, setTtlDays] = useState(30);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [drag, setDrag] = useState(false);
  const [naming, setNaming] = useState(false);
  const [newName, setNewName] = useState("");
  const [renaming, setRenaming] = useState<Renaming | null>(null);
  const [moving, setMoving] = useState<Moving | null>(null);
  const [previewId, setPreviewId] = useState<number | null>(null);
  const [thumbs, setThumbs] = useState<Record<number, string>>({});

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

  const refresh = useCallback(async () => {
    setError(null);
    setReloadTick((n) => n + 1);
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
  }, [folderId, inTrash]);

  useEffect(() => {
    if (user) {
      void refresh();
    }
  }, [user, refresh]);

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
      if (event.key !== "/" || event.ctrlKey || event.metaKey || event.altKey) {
        return;
      }
      if (previewId !== null || moving !== null || naming || renaming !== null) {
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
  }, [previewId, moving, naming, renaming]);

  useEffect(() => {
    const cache = blobCache.current;
    return () => {
      cache.forEach((url) => URL.revokeObjectURL(url));
      cache.clear();
    };
  }, []);

  const needBlob = useCallback(async (id: number) => {
    const url = await loadBlobUrl(blobCache.current, id);
    setThumbs((prev) => (prev[id] === url ? prev : { ...prev, [id]: url }));
    return url;
  }, []);

  if (ready && !user) {
    return <Navigate to="/login" replace />;
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
    if (items.length === 0) {
      return;
    }
    setError(null);
    const cache = new Map<string, DriveFolder[]>();
    const total = items.length;
    setBusy(`Enviando 0/${total}…`);
    try {
      let done = 0;
      for (const item of items) {
        const dest = await ensureFolderPath(folderId, item.dirs, cache);
        if (item.file) {
          await uploadFile(item.file, dest);
        }
        done += 1;
        setBusy(`Enviando ${done}/${total}…`);
      }
      await refresh();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Falha no envio.");
      await refresh();
    } finally {
      setBusy(null);
    }
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
    if (searchScope === "global" || typeFilter === "folder") {
      setTypeFilter("all");
    }
    setPreviewId(null);
    setCrumbs((prev) => [...prev, folder]);
    setFolderId(folder.id);
  }

  function openContainingFolder(targetFolderId: number | null) {
    clearFilters();
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
    try {
      if (moving.kind === "folder") {
        await moveFolder(moving.id, dest);
      } else {
        await moveFile(moving.id, dest);
      }
      setMoving(null);
      await refresh();
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
        <div>
          <p className="eyebrow">Drive local</p>
          <h1>{title}</h1>
          {inTrash ? (
            <p className="hint">Itens somem após {ttlDays} dias.</p>
          ) : null}
        </div>
        <div className="top-actions">
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
          <p>{busy ?? "Solte arquivos ou pastas nesta pasta, ou"}</p>
          <div className="drop-actions">
            <button type="button" className="ghost" onClick={() => setNaming(true)}>
              Nova pasta
            </button>
            <button type="button" disabled={Boolean(busy)} onClick={() => inputRef.current?.click()}>
              Enviar arquivos
            </button>
            <button
              type="button"
              className="ghost"
              disabled={Boolean(busy)}
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
      </section>

      {error ? <p className="error">{error}</p> : null}

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
                            setMoving({ kind: "folder", id: folder.id, nome: folder.nome })
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
            const fileSubtitle = inTrash
              ? `${formatBytes(file.tamanho_bytes)} · ${ttlLabel(file.expires_at)}`
              : file.location
                ? `${formatBytes(file.tamanho_bytes)} · ${file.location}`
                : formatBytes(file.tamanho_bytes);
            return (
              <li key={`f-${file.id}`}>
                <div className="tile">
                  <button type="button" className="tile-hit" onClick={() => openPreview(file)}>
                    <MediaThumb file={file} blobUrl={thumbs[file.id]} onNeedBlob={needBlob} />
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
                            setMoving({ kind: "file", id: file.id, nome: file.nome })
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
          item={moving}
          startId={folderId}
          startCrumbs={crumbs}
          onCancel={() => setMoving(null)}
          onConfirm={(dest) => void onMove(dest)}
        />
      ) : null}

      {previewId !== null ? (
        <FileViewer
          files={visibleFiles}
          activeId={previewId}
          blobCache={blobCache.current}
          onNeedBlob={needBlob}
          onSelectId={setPreviewId}
          onDownload={(file) => void onDownload(file)}
          onClose={() => setPreviewId(null)}
        />
      ) : null}
    </main>
  );
}

function MovePicker({
  item,
  startId,
  startCrumbs,
  onCancel,
  onConfirm,
}: {
  item: Moving;
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
          setFolders(next.filter((folder) => !(item.kind === "folder" && folder.id === item.id)));
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
  }, [here, item]);

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

  return (
    <div className="lightbox" onClick={onCancel} role="presentation">
      <div className="lightbox-card move-card" onClick={(e) => e.stopPropagation()} role="dialog" aria-modal="true">
        <header>
          <strong>Mover “{item.nome}”</strong>
          <button type="button" className="ghost" onClick={onCancel}>
            Fechar
          </button>
        </header>
        <nav className="crumbs" aria-label="Destino">
          <button type="button" className={here === null ? "crumb current" : "crumb"} onClick={() => go(null)}>
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
