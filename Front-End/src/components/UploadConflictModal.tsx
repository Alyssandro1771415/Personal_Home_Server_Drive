import { useEffect, useMemo, useState, type FormEvent } from "react";
import type { DriveFile } from "../api";
import { cleanUploadFileName, formatBytes, localFileKind } from "../media";
import { KindMark, MediaThumb } from "./MediaThumb";

export type ConflictResolution =
  | { action: "keep_both"; newName: string }
  | { action: "overwrite" }
  | { action: "cancel" };

type Props = {
  existingFile: DriveFile;
  incomingFile: File;
  suggestedName: string;
  existingNames: ReadonlySet<string>;
  fileVersion?: number;
  onResolve: (resolution: ConflictResolution) => void;
};

export function UploadConflictModal({
  existingFile,
  incomingFile,
  suggestedName,
  existingNames,
  fileVersion,
  onResolve,
}: Props) {
  const [newName, setNewName] = useState(suggestedName);
  const [localUrl, setLocalUrl] = useState<string | null>(null);
  const [localFailed, setLocalFailed] = useState(false);

  const incomingKind = useMemo(() => localFileKind(incomingFile), [incomingFile]);
  const incomingCleanName = useMemo(
    () => cleanUploadFileName(incomingFile.name),
    [incomingFile.name],
  );

  useEffect(() => {
    setNewName(suggestedName);
  }, [suggestedName, existingFile.id, incomingFile]);

  useEffect(() => {
    setLocalFailed(false);
    if (incomingKind === "other") {
      setLocalUrl(null);
      return;
    }
    const url = URL.createObjectURL(incomingFile);
    setLocalUrl(url);
    return () => {
      URL.revokeObjectURL(url);
    };
  }, [incomingFile, incomingKind]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        onResolve({ action: "cancel" });
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onResolve]);

  const sanitizedName = newName.trim().replaceAll("/", "").replaceAll("\\", "");
  const isInvalidName =
    sanitizedName.length === 0 ||
    sanitizedName.length > 200 ||
    sanitizedName === "." ||
    sanitizedName === ".." ||
    sanitizedName.toLowerCase() === ".lixeira";
  const nameTaken = !isInvalidName && existingNames.has(sanitizedName);

  function submitKeepBoth(event?: FormEvent) {
    event?.preventDefault();
    if (isInvalidName || nameTaken) {
      return;
    }
    onResolve({ action: "keep_both", newName: sanitizedName });
  }

  return (
    <div className="lightbox conflict-backdrop" role="presentation">
      <div
        className="lightbox-card conflict-card"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="conflict-modal-title"
      >
        <header className="conflict-header">
          <div>
            <p className="eyebrow">Conflito de nome</p>
            <strong id="conflict-modal-title">
              “{existingFile.nome}” já existe neste diretório
            </strong>
          </div>
        </header>

        <p className="conflict-lead">
          Compare os dois arquivos abaixo e escolha como deseja prosseguir com este envio:
        </p>

        <div className="conflict-canvas">
          <div className="conflict-pane">
            <span className="conflict-tag">Atual no Drive</span>
            <MediaThumb file={existingFile} version={fileVersion} />
            <div className="conflict-meta">
              <strong title={existingFile.nome}>{existingFile.nome}</strong>
              <span>{formatBytes(existingFile.tamanho_bytes)}</span>
            </div>
          </div>

          <div className="conflict-pane incoming">
            <span className="conflict-tag accent">Novo arquivo</span>
            <div className={`tile-thumb kind-${incomingKind}`}>
              {localUrl && !localFailed && incomingKind === "image" ? (
                <img
                  src={localUrl}
                  alt=""
                  decoding="async"
                  onError={() => setLocalFailed(true)}
                />
              ) : localUrl && !localFailed && incomingKind === "video" ? (
                <video
                  src={`${localUrl}#t=0.4`}
                  muted
                  playsInline
                  preload="metadata"
                  onError={() => setLocalFailed(true)}
                />
              ) : (
                <KindMark kind={incomingKind} ext={incomingCleanName} />
              )}
              {incomingKind === "video" ? <span className="play-badge">▶</span> : null}
            </div>
            <div className="conflict-meta">
              <strong title={incomingCleanName}>{incomingCleanName}</strong>
              <span>{formatBytes(incomingFile.size)}</span>
            </div>
          </div>
        </div>

        <form className="conflict-rename-box" onSubmit={submitKeepBoth}>
          <label htmlFor="conflict-new-name">Novo nome para manter os dois</label>
          <div className="conflict-rename-row">
            <input
              id="conflict-new-name"
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              maxLength={200}
              autoComplete="off"
              spellCheck={false}
            />
            {sanitizedName !== suggestedName ? (
              <button
                type="button"
                className="ghost conflict-suggest-btn"
                onClick={() => setNewName(suggestedName)}
              >
                Usar sugerido
              </button>
            ) : null}
          </div>
          {nameTaken ? (
            <p className="conflict-validation">
              Esse nome já está em uso nesta pasta. Altere o nome acima para manter os dois.
            </p>
          ) : isInvalidName ? (
            <p className="conflict-validation">Informe um nome válido.</p>
          ) : null}
        </form>

        <div className="conflict-actions">
          <button
            type="button"
            disabled={isInvalidName || nameTaken}
            onClick={() => submitKeepBoth()}
          >
            Manter os dois
          </button>
          <button
            type="button"
            className="danger"
            onClick={() => onResolve({ action: "overwrite" })}
          >
            Sobrepor ao antigo
          </button>
          <button
            type="button"
            className="ghost"
            onClick={() => onResolve({ action: "cancel" })}
          >
            Desistir de upar
          </button>
        </div>
      </div>
    </div>
  );
}
