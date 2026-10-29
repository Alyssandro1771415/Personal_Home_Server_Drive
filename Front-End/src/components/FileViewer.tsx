import { useCallback, useEffect, useRef, useState, type TouchEvent } from "react";
import { ApiError, type DriveFile } from "../api";
import { canAutoThumb, extensionOf, fileKind, formatBytes } from "../media";

type Props = {
  files: DriveFile[];
  activeId: number;
  blobCache: Map<number, string>;
  onNeedBlob: (id: number) => Promise<string>;
  onSelectId: (id: number) => void;
  onDownload: (file: DriveFile) => void;
  onClose: () => void;
};

type LoadState = {
  fileId: number;
  url: string | null;
  error: string | null;
};

type TouchTrack = {
  startX: number;
  startY: number;
  locked: "x" | "y" | null;
  blocked: boolean;
};

const SWIPE_THRESHOLD_PX = 48;
const VIDEO_CONTROLS_GUARD_PX = 68;

export function FileViewer({
  files,
  activeId,
  blobCache,
  onNeedBlob,
  onSelectId,
  onDownload,
  onClose,
}: Props) {
  const index = files.findIndex((item) => item.id === activeId);
  const file = index >= 0 ? files[index] : undefined;
  const kind = file ? fileKind(file) : "other";
  const hasPrev = index > 0;
  const hasNext = index >= 0 && index < files.length - 1;

  const [loadState, setLoadState] = useState<LoadState | null>(null);
  const [dragX, setDragX] = useState(0);
  const touchRef = useRef<TouchTrack | null>(null);
  const videoRef = useRef<HTMLVideoElement>(null);

  const cachedUrl = file && kind !== "other" ? (blobCache.get(file.id) ?? null) : null;
  const resolvedUrl = cachedUrl ?? (file && loadState?.fileId === file.id ? loadState.url : null);
  const loadError = file && loadState?.fileId === file.id ? loadState.error : null;
  const loading = Boolean(file && kind !== "other" && !resolvedUrl && !loadError);

  const goPrev = useCallback(() => {
    if (index > 0) {
      onSelectId(files[index - 1].id);
    }
  }, [files, index, onSelectId]);

  const goNext = useCallback(() => {
    if (index >= 0 && index < files.length - 1) {
      onSelectId(files[index + 1].id);
    }
  }, [files, index, onSelectId]);

  useEffect(() => {
    if (!file) {
      onClose();
    }
  }, [file, onClose]);

  useEffect(() => {
    setDragX(0);
    touchRef.current = null;
  }, [activeId]);

  useEffect(() => {
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = prevOverflow;
    };
  }, []);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        goPrev();
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        goNext();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [goPrev, goNext, onClose]);

  useEffect(() => {
    if (!file || kind === "other") {
      return;
    }
    if (blobCache.has(file.id)) {
      return;
    }
    let cancelled = false;
    void onNeedBlob(file.id)
      .then((url) => {
        if (!cancelled) {
          setLoadState({ fileId: file.id, url, error: null });
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setLoadState({
            fileId: file.id,
            url: null,
            error: err instanceof ApiError ? err.message : "Falha ao abrir o arquivo.",
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [file, kind, blobCache, onNeedBlob]);

  useEffect(() => {
    if (index < 0) {
      return;
    }
    const neighbors = [files[index - 1], files[index + 1]];
    for (const neighbor of neighbors) {
      if (neighbor && canAutoThumb(neighbor) && !blobCache.has(neighbor.id)) {
        void onNeedBlob(neighbor.id).catch(() => {});
      }
    }
  }, [files, index, blobCache, onNeedBlob]);

  useEffect(() => {
    const el = videoRef.current;
    return () => {
      el?.pause();
    };
  }, [activeId, resolvedUrl]);

  if (!file) {
    return null;
  }

  function onTouchStart(event: TouchEvent<HTMLDivElement>) {
    if (files.length <= 1 || event.touches.length !== 1) {
      touchRef.current = { startX: 0, startY: 0, locked: null, blocked: true };
      setDragX(0);
      return;
    }
    const touch = event.touches[0];
    const target = event.target;
    if (target instanceof HTMLVideoElement) {
      const rect = target.getBoundingClientRect();
      if (touch.clientY > rect.bottom - VIDEO_CONTROLS_GUARD_PX) {
        touchRef.current = { startX: 0, startY: 0, locked: null, blocked: true };
        return;
      }
    }
    touchRef.current = {
      startX: touch.clientX,
      startY: touch.clientY,
      locked: null,
      blocked: false,
    };
  }

  function onTouchMove(event: TouchEvent<HTMLDivElement>) {
    const track = touchRef.current;
    if (!track || track.blocked) {
      return;
    }
    if (event.touches.length !== 1) {
      track.blocked = true;
      setDragX(0);
      return;
    }
    const touch = event.touches[0];
    const dx = touch.clientX - track.startX;
    const dy = touch.clientY - track.startY;
    if (track.locked === null) {
      if (Math.abs(dx) < 8 && Math.abs(dy) < 8) {
        return;
      }
      track.locked = Math.abs(dx) > Math.abs(dy) * 1.15 ? "x" : "y";
    }
    if (track.locked === "x") {
      const atEdge = (dx > 0 && !hasPrev) || (dx < 0 && !hasNext);
      setDragX(atEdge ? dx * 0.18 : dx * 0.55);
    }
  }

  function onTouchEnd(event: TouchEvent<HTMLDivElement>) {
    const track = touchRef.current;
    touchRef.current = null;
    setDragX(0);
    if (!track || track.blocked || track.locked !== "x" || event.changedTouches.length === 0) {
      return;
    }
    const dx = event.changedTouches[0].clientX - track.startX;
    if (dx <= -SWIPE_THRESHOLD_PX && hasNext) {
      goNext();
    } else if (dx >= SWIPE_THRESHOLD_PX && hasPrev) {
      goPrev();
    }
  }

  function onTouchCancel() {
    touchRef.current = null;
    setDragX(0);
  }

  const extLabel = (extensionOf(file.nome) || "FILE").toUpperCase().slice(0, 6);

  return (
    <div className="lightbox" onClick={onClose} role="presentation">
      <div
        className="lightbox-card viewer-card"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={file.nome}
      >
        <header className="viewer-header">
          <div className="viewer-title">
            <strong title={file.nome}>{file.nome}</strong>
            <span>
              {files.length > 1 ? `${index + 1} / ${files.length} · ` : ""}
              {formatBytes(file.tamanho_bytes)}
              {file.location ? ` · ${file.location}` : ""}
            </span>
          </div>
          <div className="viewer-actions">
            <button type="button" className="ghost" onClick={() => onDownload(file)}>
              Baixar
            </button>
            <button type="button" className="ghost" onClick={onClose}>
              Fechar
            </button>
          </div>
        </header>

        <div
          className="viewer-stage"
          onTouchStart={onTouchStart}
          onTouchMove={onTouchMove}
          onTouchEnd={onTouchEnd}
          onTouchCancel={onTouchCancel}
        >
          {files.length > 1 ? (
            <button
              type="button"
              className="viewer-nav prev"
              onClick={goPrev}
              disabled={!hasPrev}
              aria-label="Arquivo anterior"
              title="Anterior (←)"
            >
              <ChevronLeft />
            </button>
          ) : null}

          <div
            className="viewer-media"
            style={{
              transform: dragX !== 0 ? `translateX(${dragX}px)` : undefined,
              transition: dragX !== 0 ? "none" : "transform 0.18s ease",
            }}
          >
            {loading ? (
              <div className="viewer-fallback">
                <span className="glyph">CARREGANDO…</span>
              </div>
            ) : loadError ? (
              <div className="viewer-fallback">
                <p className="error">{loadError}</p>
                <button type="button" onClick={() => onDownload(file)}>
                  Baixar arquivo
                </button>
              </div>
            ) : kind === "image" && resolvedUrl ? (
              <img key={file.id} src={resolvedUrl} alt={file.nome} draggable={false} />
            ) : kind === "video" && resolvedUrl ? (
              <video
                key={file.id}
                ref={videoRef}
                src={resolvedUrl}
                controls
                autoPlay
                loop
                playsInline
                onEnded={(e) => {
                  const el = e.currentTarget;
                  el.currentTime = 0;
                  void el.play().catch(() => {});
                }}
              />
            ) : (
              <div className="viewer-fallback">
                <span className="viewer-badge">{extLabel}</span>
                <strong>{file.nome}</strong>
                <span className="muted">{formatBytes(file.tamanho_bytes)}</span>
                <button type="button" onClick={() => onDownload(file)}>
                  Baixar arquivo
                </button>
              </div>
            )}
          </div>

          {files.length > 1 ? (
            <button
              type="button"
              className="viewer-nav next"
              onClick={goNext}
              disabled={!hasNext}
              aria-label="Próximo arquivo"
              title="Próximo (→)"
            >
              <ChevronRight />
            </button>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function ChevronLeft() {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" aria-hidden="true">
      <path
        d="M15 5l-7 7 7 7"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function ChevronRight() {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" aria-hidden="true">
      <path
        d="M9 5l7 7-7 7"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
