import { useEffect, useRef, useState } from "react";
import { fetchFileBlob, type DriveFile } from "../api";
import { canAutoThumb, fileKind } from "../media";

type Props = {
  file: DriveFile;
  blobUrl?: string;
  onNeedBlob: (id: number) => Promise<string>;
};

export function MediaThumb({ file, blobUrl, onNeedBlob }: Props) {
  const kind = fileKind(file);
  const hostRef = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const [failed, setFailed] = useState(false);

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
      { rootMargin: "80px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!visible || blobUrl || !canAutoThumb(file) || failed) {
      return;
    }
    let cancelled = false;
    void onNeedBlob(file.id).catch(() => {
      if (!cancelled) {
        setFailed(true);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [visible, blobUrl, file, failed, onNeedBlob]);

  return (
    <div ref={hostRef} className={`tile-thumb kind-${kind}`}>
      {blobUrl && kind === "image" ? (
        <img src={blobUrl} alt="" decoding="async" />
      ) : blobUrl && kind === "video" ? (
        <video src={`${blobUrl}#t=0.4`} muted playsInline preload="metadata" />
      ) : (
        <KindMark kind={kind} ext={file.nome} />
      )}
      {kind === "video" ? <span className="play-badge">▶</span> : null}
    </div>
  );
}

function KindMark({ kind, ext }: { kind: "image" | "video" | "other"; ext: string }) {
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
