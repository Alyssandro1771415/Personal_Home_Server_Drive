import { useEffect, useRef, useState } from "react";
import { fetchFileBlob, fileContentUrl, type DriveFile } from "../api";
import { canAutoThumb, fileKind } from "../media";

type Props = {
  file: DriveFile;
  blobUrl?: string;
  version?: number;
  onNeedBlob?: (id: number) => Promise<string>;
};

export function MediaThumb({ file, blobUrl, version }: Props) {
  const kind = fileKind(file);
  const hostRef = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const [failed, setFailed] = useState(false);

  const cacheVersion = version ?? file.tamanho_bytes;

  useEffect(() => {
    setFailed(false);
  }, [file.id, file.tamanho_bytes, version]);

  useEffect(() => {
    const el = hostRef.current;
    if (!el) {
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setVisible(true);
        }
      },
      { rootMargin: "120px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const allowPreview = visible && !failed && canAutoThumb(file);
  const mediaSrc = allowPreview
    ? (blobUrl ?? fileContentUrl(file.id, { inline: true, version: cacheVersion }))
    : null;

  return (
    <div ref={hostRef} className={`tile-thumb kind-${kind}`}>
      {mediaSrc && kind === "image" ? (
        <img
          src={mediaSrc}
          alt=""
          decoding="async"
          loading="lazy"
          onError={() => setFailed(true)}
        />
      ) : mediaSrc && kind === "video" ? (
        <video
          src={`${mediaSrc}#t=0.4`}
          muted
          playsInline
          preload="metadata"
          onError={() => setFailed(true)}
        />
      ) : (
        <KindMark kind={kind} ext={file.nome} />
      )}
      {kind === "video" ? <span className="play-badge">▶</span> : null}
    </div>
  );
}

export function KindMark({
  kind,
  ext,
}: {
  kind: "image" | "video" | "other";
  ext: string;
}) {
  const suffix = ext.includes(".") ? ext.slice(ext.lastIndexOf(".") + 1).toUpperCase() : "FILE";
  if (kind === "image") {
    return <span className="glyph">IMG</span>;
  }
  if (kind === "video") {
    return <span className="glyph">VID</span>;
  }
  return <span className="glyph">{suffix.slice(0, 4)}</span>;
}

export async function loadBlobUrl(
  cache: Map<number, string>,
  id: number,
): Promise<string> {
  const hit = cache.get(id);
  if (hit) {
    return hit;
  }
  const blob = await fetchFileBlob(id);
  const url = URL.createObjectURL(blob);
  cache.set(id, url);
  return url;
}
