import { useCallback, useEffect, useRef, useState, type DragEvent, type FormEvent } from "react";
import { Navigate } from "react-router-dom";
import {
  ApiError,
  createFolder,
  downloadFile,
  folderPath,
  listFiles,
  listFolders,
  uploadFile,
  type DriveFile,
  type DriveFolder,
} from "../api";
import { useAuth } from "../auth";
import { MediaThumb, loadBlobUrl } from "../components/MediaThumb";
import { fileKind } from "../media";

function formatBytes(n: number): string {
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

type Preview = {
  file: DriveFile;
  url: string;
};

export function DrivePage() {
  const { user, ready, logout } = useAuth();
  const [folderId, setFolderId] = useState<number | null>(null);
  const [crumbs, setCrumbs] = useState<DriveFolder[]>([]);
  const [folders, setFolders] = useState<DriveFolder[]>([]);
  const [files, setFiles] = useState<DriveFile[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [drag, setDrag] = useState(false);
  const [naming, setNaming] = useState(false);
  const [newName, setNewName] = useState("");
  const [preview, setPreview] = useState<Preview | null>(null);
  const [thumbs, setThumbs] = useState<Record<number, string>>({});
  const inputRef = useRef<HTMLInputElement>(null);
  const blobCache = useRef(new Map<number, string>());

  const refresh = useCallback(async () => {
    setError(null);
    try {
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
  }, [folderId]);

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

  async function sendFiles(list: FileList | File[]) {
    const batch = Array.from(list);
    if (batch.length === 0) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      for (const file of batch) {
        await uploadFile(file, folderId);
      }
      await refresh();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Falha no envio.");
    } finally {
      setBusy(false);
    }
  }

  function onDrop(event: DragEvent<HTMLElement>) {
    event.preventDefault();
    setDrag(false);
    void sendFiles(event.dataTransfer.files);
  }

  async function onDownload(file: DriveFile) {
    setError(null);
    try {
      await downloadFile(file.id, file.nome);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Falha no download.");
    }
  }

  async function openPreview(file: DriveFile) {
    const kind = fileKind(file);
    if (kind === "other") {
      await onDownload(file);
      return;
    }
    setError(null);
    try {
      const url = await needBlob(file.id);
      setPreview({ file, url });
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Falha ao abrir o arquivo.");
    }
  }

  function goTo(id: number | null) {
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
    setCrumbs((prev) => [...prev, folder]);
    setFolderId(folder.id);
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

  if (!ready || !user) {
    return (
      <main className="drive-shell">
        <p className="muted">Abrindo cofre…</p>
      </main>
    );
  }

  const empty = folders.length === 0 && files.length === 0;

  return (
    <main className="drive-shell wide">
      <header className="topbar">
        <div>
          <p className="eyebrow">Home Drive</p>
          <h1>{folderId === null ? `Cofre de ${user.name}` : crumbs.at(-1)?.nome ?? "Pasta"}</h1>
        </div>
        <button type="button" className="ghost" onClick={() => void logout()}>
          Sair
        </button>
      </header>

      <nav className="crumbs" aria-label="Caminho">
        <button type="button" className={folderId === null ? "crumb current" : "crumb"} onClick={() => goTo(null)}>
          Início
        </button>
        {crumbs.map((crumb) => (
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
        ))}
      </nav>

      <section
        className={drag ? "dropzone drag" : "dropzone"}
        onDragOver={(e) => {
          e.preventDefault();
          setDrag(true);
        }}
        onDragLeave={() => setDrag(false)}
        onDrop={onDrop}
      >
        <p>Solte arquivos nesta pasta, ou</p>
        <div className="drop-actions">
          <button type="button" className="ghost" onClick={() => setNaming(true)}>
            Nova pasta
          </button>
          <button type="button" disabled={busy} onClick={() => inputRef.current?.click()}>
            {busy ? "Enviando…" : "Enviar arquivos"}
          </button>
        </div>
        <input
          ref={inputRef}
          type="file"
          multiple
          hidden
          onChange={(e) => {
            if (e.target.files) {
              void sendFiles(e.target.files);
              e.target.value = "";
            }
          }}
        />
      </section>

      {naming ? (
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

      {error ? <p className="error">{error}</p> : null}

      {empty ? (
        <p className="empty">Pasta vazia. Crie um diretório ou envie um arquivo.</p>
      ) : (
        <ul className="drive-grid">
          {folders.map((folder) => (
            <li key={`d-${folder.id}`}>
              <button type="button" className="tile folder" onClick={() => openFolder(folder)}>
                <div className="tile-thumb kind-folder">
                  <FolderMark />
                </div>
                <div className="tile-meta">
                  <strong>{folder.nome}</strong>
                  <span>Pasta</span>
                </div>
              </button>
            </li>
          ))}
          {files.map((file) => (
            <li key={`f-${file.id}`}>
              <div className="tile">
                <button type="button" className="tile-hit" onClick={() => void openPreview(file)}>
                  <MediaThumb file={file} blobUrl={thumbs[file.id]} onNeedBlob={needBlob} />
                  <div className="tile-meta">
                    <strong title={file.nome}>{file.nome}</strong>
                    <span>{formatBytes(file.tamanho_bytes)}</span>
                  </div>
                </button>
                <button type="button" className="tile-dl" onClick={() => void onDownload(file)}>
                  Baixar
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {preview ? (
        <div className="lightbox" onClick={() => setPreview(null)} role="presentation">
          <div className="lightbox-card" onClick={(e) => e.stopPropagation()} role="dialog" aria-modal="true">
            <header>
              <strong>{preview.file.nome}</strong>
              <button type="button" className="ghost" onClick={() => setPreview(null)}>
                Fechar
              </button>
            </header>
            {fileKind(preview.file) === "image" ? (
              <img src={preview.url} alt={preview.file.nome} />
            ) : (
              <video src={preview.url} controls autoPlay />
            )}
          </div>
        </div>
      ) : null}
    </main>
  );
}

function FolderMark() {
  return (
    <svg viewBox="0 0 64 48" className="folder-svg" aria-hidden="true">
      <path
        d="M4 12c0-3 2-5 5-5h16l6 6h24c3 0 5 2 5 5v21c0 3-2 5-5 5H9c-3 0-5-2-5-5V12z"
        fill="#d4a054"
      />
      <path d="M4 20h56v19c0 3-2 5-5 5H9c-3 0-5-2-5-5V20z" fill="#b8893f" />
    </svg>
  );
}
