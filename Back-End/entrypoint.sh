#!/bin/sh
set -eu

PUID="${PUID:-1000}"
PGID="${PGID:-1000}"
STORAGE_DIR="${STORAGE_ROOT:-/data/storage}"

mkdir -p "$STORAGE_DIR"

if [ "$(id -u)" = "0" ]; then
    chown -R "${PUID}:${PGID}" "$STORAGE_DIR"
    exec gosu "${PUID}:${PGID}" /usr/local/bin/drive "$@"
fi

exec /usr/local/bin/drive "$@"
