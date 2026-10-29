#!/bin/sh
set -eu

BACKUP_DIR="/backups"
KEEP_DAYS="${BACKUP_KEEP_DAYS:-14}"
INTERVAL_SECONDS="$(( ${BACKUP_INTERVAL_HOURS:-24} * 3600 ))"

mkdir -p "$BACKUP_DIR"

until pg_isready -h "$PGHOST" -p "${PGPORT:-5432}" -U "$POSTGRES_USER" -d "$POSTGRES_DB" >/dev/null 2>&1; do
    echo "[db-backup] Aguardando PostgreSQL..."
    sleep 3
done

while true; do
    STAMP="$(date +%Y-%m-%d_%H-%M-%S)"
    OUT="${BACKUP_DIR}/personal_drive_${STAMP}.sql.gz"
    TMP="${OUT}.tmp"

    echo "[db-backup] Gerando backup em ${OUT}..."
    if pg_dump -h "$PGHOST" -p "${PGPORT:-5432}" -U "$POSTGRES_USER" -d "$POSTGRES_DB" | gzip -9 > "$TMP"; then
        mv "$TMP" "$OUT"
        find "$BACKUP_DIR" -name 'personal_drive_*.sql.gz' -type f -mtime +"$KEEP_DAYS" -delete || true
        echo "[db-backup] Backup concluído."
    else
        rm -f "$TMP"
        echo "[db-backup] Falha ao gerar backup." >&2
    fi

    sleep "$INTERVAL_SECONDS"
done
