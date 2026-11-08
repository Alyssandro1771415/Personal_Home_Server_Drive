import { formatTransferBytes } from "../media";
import type { PersistedUploadState, UploadTaskRecord } from "../uploadPersistence";

type Props = {
  state: PersistedUploadState;
  active: boolean;
  onToggleMinimize: () => void;
  onCancelActive: () => void;
  onClearHistory: () => void;
};

function statusLabel(task: UploadTaskRecord): string {
  switch (task.status) {
    case "uploading": {
      if (task.size <= 0) {
        return "Enviando…";
      }
      const pct = Math.min(100, Math.round((task.loadedBytes / task.size) * 100));
      return `${formatTransferBytes(task.loadedBytes)} / ${formatTransferBytes(task.size)} · ${pct}%`;
    }
    case "conflict":
      return "Aguardando decisão…";
    case "queued":
      return "Na fila";
    case "done":
      if (task.resolution === "overwritten") {
        return "Sobrescrito";
      }
      if (task.resolution === "kept_both") {
        return `Salvo como ${task.finalName ?? task.name}`;
      }
      return "Concluído";
    case "skipped":
      return "Cancelado";
    case "interrupted":
      return "Interrompido";
    case "error":
      return task.errorMessage ?? "Falha";
  }
}

export function UploadPanel({
  state,
  active,
  onToggleMinimize,
  onCancelActive,
  onClearHistory,
}: Props) {
  const { tasks, totalUploadedCount, minimized } = state;

  if (tasks.length === 0 && totalUploadedCount === 0) {
    return null;
  }

  const activeTasks = tasks.filter(
    (t) => t.status !== "skipped" && t.status !== "interrupted",
  );
  const doneInList = tasks.filter((t) => t.status === "done").length;
  const totalInList = tasks.length;

  let totalBytes = 0;
  let loadedBytes = 0;
  for (const task of activeTasks) {
    const size = Math.max(0, task.size);
    totalBytes += size;
    if (task.status === "done") {
      loadedBytes += size;
    } else if (task.status === "uploading" || task.status === "conflict") {
      loadedBytes += Math.min(size, Math.max(0, task.loadedBytes));
    }
  }

  const overallPct = active
    ? totalBytes > 0
      ? Math.min(100, Math.max(0, Math.round((loadedBytes / totalBytes) * 100)))
      : totalInList > 0
        ? Math.min(100, Math.round((doneInList / totalInList) * 100))
        : 0
    : 100;

  const currentTask = tasks.find(
    (t) => t.status === "uploading" || t.status === "conflict",
  );

  const headline = active
    ? currentTask?.status === "conflict"
      ? `Conflito em "${currentTask.name}"`
      : `Enviando ${doneInList}/${totalInList}…`
    : totalInList > 0
      ? `Envio finalizado · ${doneInList}/${totalInList} concluídos`
      : "Histórico de envio";

  const counterLabel =
    totalUploadedCount === 1
      ? "1 item upado"
      : `${totalUploadedCount} itens upados`;

  return (
    <section className="upload-panel" aria-label="Progresso e contagem de uploads">
      <div className="upload-panel-head">
        <div className="upload-panel-summary">
          <div className="upload-panel-titles">
            <strong className="upload-headline">{headline}</strong>
            <span className="upload-counter-badge" title="Contagem mantida mesmo ao sair ou recarregar a página">
              {counterLabel}
            </span>
          </div>
          {active && totalBytes > 0 ? (
            <span className="upload-bytes-meta">
              {formatTransferBytes(loadedBytes)} / {formatTransferBytes(totalBytes)} · {overallPct}%
            </span>
          ) : null}
        </div>

        <div className="upload-panel-controls">
          {active ? (
            <button
              type="button"
              className="danger upload-ctrl-btn"
              onClick={onCancelActive}
            >
              Cancelar
            </button>
          ) : (
            <button
              type="button"
              className="ghost upload-ctrl-btn"
              onClick={onClearHistory}
              title="Zerar contador e limpar histórico"
            >
              Limpar
            </button>
          )}
          {tasks.length > 0 ? (
            <button
              type="button"
              className="ghost upload-ctrl-btn"
              onClick={onToggleMinimize}
              aria-expanded={!minimized}
            >
              {minimized ? "Detalhes" : "Ocultar"}
            </button>
          ) : null}
        </div>
      </div>

      {active || tasks.length > 0 ? (
        <div
          className="upload-bar-track"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={overallPct}
        >
          <div
            className={active ? "upload-bar-fill active" : "upload-bar-fill"}
            style={{ width: `${overallPct}%` }}
          />
        </div>
      ) : null}

      {!minimized && tasks.length > 0 ? (
        <ul className="upload-task-list">
          {tasks
            .slice()
            .reverse()
            .slice(0, 40)
            .map((task) => {
              const itemPct =
                task.status === "done"
                  ? 100
                  : task.size > 0
                    ? Math.min(100, Math.round((task.loadedBytes / task.size) * 100))
                    : 0;
              return (
                <li key={task.id} className={`upload-task-item status-${task.status}`}>
                  <div className="upload-task-row">
                    <span className="upload-task-name" title={task.finalName ?? task.name}>
                      {task.finalName ?? task.name}
                    </span>
                    <span className="upload-task-status">{statusLabel(task)}</span>
                  </div>
                  {task.status === "uploading" ? (
                    <div className="upload-item-track">
                      <div
                        className="upload-item-fill"
                        style={{ width: `${itemPct}%` }}
                      />
                    </div>
                  ) : null}
                </li>
              );
            })}
        </ul>
      ) : null}
    </section>
  );
}
