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
  trashFile,
  trashFolder,
  uploadFile,
  type DriveFile,
  type DriveFolder,
} from "../api";
import { useAuth } from "../auth";
import { FileViewer } from "../components/FileViewer";
import { MediaThumb, loadBlobUrl } from "../components/MediaThumb";
import { formatBytes } from "../media";
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
  const inputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);
  const blobCache = useRef(new Map<number, string>());

  const refresh = useCallback(async () => {
    setError(null);
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
    setPreviewId(null);
    setCrumbs((prev) => [...prev, folder]);
    setFolderId(folder.id);
  }

  function openTrash() {
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

  const empty = folders.length === 0 && files.length === 0;
  const title = inTrash
    ? "Lixeira"
    : folderId === null
      ? `Drive de ${user.name}`
      : (crumbs.at(-1)?.nome ?? "Pasta");

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
        <button type="button" className={!inTrash && folderId === null ? "crumb current" : "crumb"} onClick={() => goTo(null)}>
          Início
        </button>
        {!inTrash
          ? crumbs.map((crumb) => (
              <span key={crumb.id} className="crumb-wrap">
                <span className="sep" aria-hidden="true">
                  /
                </span>
                <button
                  type="button"
                  className={crumb.id === folderId ? "crumb current" : "crumb"}
                  onClick={() => goTo(crumb.id)}
                >
                  {crumb.nome}
                </button>
              </span>
            ))
          : (
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

      {error ? <p className="error">{error}</p> : null}

      {empty ? (
        <p className="empty">
          {inTrash ? "Lixeira vazia." : "Pasta vazia. Crie um diretório ou envie um arquivo."}
        </p>
      ) : (
        <ul className="drive-grid">
          {folders.map((folder) => (
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
                    <span>{inTrash ? ttlLabel(folder.expires_at) : "Pasta"}</span>
                  </div>
                </button>
                <div className="tile-actions">
                  {inTrash ? (
                    <>
                      <button type="button" className="ghost" onClick={() => void onRestore("folder", folder.id)}>
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
                        onClick={() => setRenaming({ kind: "folder", id: folder.id, nome: folder.nome })}
                      >
                        Renomear
                      </button>
                      <button
                        type="button"
                        className="ghost"
                        onClick={() => setMoving({ kind: "folder", id: folder.id, nome: folder.nome })}
                      >
                        Mover
                      </button>
                      <button type="button" className="danger" onClick={() => void onTrash("folder", folder.id)}>
                        Lixeira
                      </button>
                    </>
                  )}
                </div>
              </div>
            </li>
          ))}
          {files.map((file) => (
            <li key={`f-${file.id}`}>
              <div className="tile">
                <button type="button" className="tile-hit" onClick={() => openPreview(file)}>
                  <MediaThumb file={file} blobUrl={thumbs[file.id]} onNeedBlob={needBlob} />
                  <div className="tile-meta">
                    <strong title={file.nome}>{file.nome}</strong>
                    <span>
                      {inTrash
                        ? `${formatBytes(file.tamanho_bytes)} · ${ttlLabel(file.expires_at)}`
                        : formatBytes(file.tamanho_bytes)}
                    </span>
                  </div>
                </button>
                <div className="tile-actions">
                  {inTrash ? (
                    <>
                      <button type="button" className="ghost" onClick={() => void onRestore("file", file.id)}>
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
                      <button type="button" className="ghost" onClick={() => void onDownload(file)}>
                        Baixar
                      </button>
                      <button
                        type="button"
                        className="ghost"
                        onClick={() => setRenaming({ kind: "file", id: file.id, nome: file.nome })}
                      >
                        Renomear
                      </button>
                      <button
                        type="button"
                        className="ghost"
                        onClick={() => setMoving({ kind: "file", id: file.id, nome: file.nome })}
                      >
                        Mover
                      </button>
                      <button type="button" className="danger" onClick={() => void onTrash("file", file.id)}>
                        Lixeira
                      </button>
                    </>
                  )}
                </div>
              </div>
            </li>
          ))}
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
          files={files}
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
          <button type="button" className="ghost" onClick={onCancel}>
            Cancelar
          </button>
        </div>
      </div>
    </div>
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
