#!/bin/sh
# Reverte os serviços backend e frontend para a release anterior (ou para uma tag específica)
# sem apagar volumes persistentes, banco de dados ou arquivos dos usuários.
#
# Uso no servidor:
#   ./scripts/rollback.sh                  # volta para a versão em .release.prev.env
#   ./scripts/rollback.sh sha-<commit_sha> # volta para uma tag específica do GHCR

set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
DEPLOY_PATH="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"

ENV_FILE="$DEPLOY_PATH/.env"
CUR_FILE="$DEPLOY_PATH/.release.env"
PREV_FILE="$DEPLOY_PATH/.release.prev.env"

log() {
    printf '[rollback %s] %s\n' "$(date -u +%H:%M:%S)" "$*"
}

fail() {
    printf '[rollback ERRO] %s\n' "$*" >&2
    exit 1
}

read_env_key() {
    key="$1"
    file="$2"
    default_val="${3:-}"
    if [ ! -f "$file" ]; then
        printf '%s' "$default_val"
        return 0
    fi
    val="$(awk -F= -v k="$key" '
        /^[[:space:]]*#/ { next }
        /^[[:space:]]*$/ { next }
        {
            lhs = $1
            sub(/^[[:space:]]*export[[:space:]]+/, "", lhs)
            gsub(/^[[:space:]]+|[[:space:]]+$/, "", lhs)
            if (lhs == k) {
                rhs = substr($0, index($0, "=") + 1)
                gsub(/^[[:space:]]+|[[:space:]]+$/, "", rhs)
                if ((rhs ~ /^".*"$/) || (rhs ~ /^'\''.*'\''$/)) {
                    rhs = substr(rhs, 2, length(rhs) - 2)
                }
                result = rhs
            }
        }
        END { print result }
    ' "$file")"
    if [ -n "$val" ]; then
        printf '%s' "$val"
    else
        printf '%s' "$default_val"
    fi
}

if [ ! -f "$ENV_FILE" ]; then
    fail "Arquivo '$ENV_FILE' nao encontrado."
fi

TARGET_TAG="${1:-}"
TARGET_PREFIX=""

if [ -n "$TARGET_TAG" ]; then
    TARGET_PREFIX="$(read_env_key GHCR_IMAGE_PREFIX "$CUR_FILE" "")"
    if [ -z "$TARGET_PREFIX" ]; then
        TARGET_PREFIX="$(read_env_key GHCR_IMAGE_PREFIX "$PREV_FILE" "")"
    fi
    [ -n "$TARGET_PREFIX" ] || fail "GHCR_IMAGE_PREFIX nao encontrado em .release.env. Exporte GHCR_IMAGE_PREFIX antes de rodar."
else
    [ -f "$PREV_FILE" ] || fail "Arquivo '$PREV_FILE' nao encontrado. Informe explicitamente a tag desejada (ex.: ./scripts/rollback.sh sha-<commit>)."
    TARGET_PREFIX="$(read_env_key GHCR_IMAGE_PREFIX "$PREV_FILE" "")"
    TARGET_TAG="$(read_env_key IMAGE_TAG "$PREV_FILE" "")"
    [ -n "$TARGET_PREFIX" ] && [ -n "$TARGET_TAG" ] || fail "Metadados incompletos em '$PREV_FILE'."
fi

USE_SUDO=0
if docker info >/dev/null 2>&1; then
    USE_SUDO=0
elif command -v sudo >/dev/null 2>&1 && sudo -n docker info >/dev/null 2>&1; then
    USE_SUDO=1
else
    fail "Sem permissao nao-interativa para executar comandos Docker."
fi

run_docker() {
    if [ "$USE_SUDO" -eq 1 ]; then
        sudo -n docker "$@"
    else
        docker "$@"
    fi
}

umask 077
TMP_ENV="$(mktemp /tmp/pd-rollback.XXXXXX)"
trap 'rm -f "$TMP_ENV"' EXIT INT TERM

cat "$ENV_FILE" > "$TMP_ENV"
printf '\nGHCR_IMAGE_PREFIX=%s\nIMAGE_TAG=%s\n' "$TARGET_PREFIX" "$TARGET_TAG" >> "$TMP_ENV"

run_compose() {
    run_docker compose \
        --project-directory "$DEPLOY_PATH" \
        --env-file "$TMP_ENV" \
        -f "$DEPLOY_PATH/docker-compose.yml" \
        -f "$DEPLOY_PATH/docker-compose.prod.yml" \
        "$@"
}

# Faz pull apenas se alguma imagem não estiver em cache local no servidor
for svc in backend frontend; do
    img="${TARGET_PREFIX}/${svc}:${TARGET_TAG}"
    if ! run_docker image inspect "$img" >/dev/null 2>&1; then
        log "Imagem $img nao esta no cache local; baixando do GHCR..."
        run_compose pull "$svc"
    fi
done

log "Restaurando servicos para ${TARGET_PREFIX}/{backend,frontend}:${TARGET_TAG}..."
run_compose up -d --no-build --remove-orphans

APP_PORT="$(read_env_key APP_PORT "$ENV_FILE" "5174")"
BACKEND_PORT="$(read_env_key BACKEND_PORT "$ENV_FILE" "5001")"
POSTGRES_USER="$(read_env_key POSTGRES_USER "$ENV_FILE" "postgres")"
POSTGRES_DB="$(read_env_key POSTGRES_DB "$ENV_FILE" "personal_drive")"

http_get() {
    url="$1"
    if command -v curl >/dev/null 2>&1; then
        curl -fsS --max-time 5 "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- -T 5 "$url"
    else
        return 127
    fi
}

elapsed=0
while [ "$elapsed" -lt 60 ]; do
    ok=1
    for cname in personal-drive-postgres personal-drive-backend personal-drive-frontend; do
        st="$(run_docker inspect --format '{{.State.Status}}:{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$cname" 2>/dev/null || echo 'missing:none')"
        if [ "$st" != "running:healthy" ]; then
            ok=0
            break
        fi
    done

    if [ "$ok" -eq 1 ] && run_docker exec personal-drive-postgres pg_isready -U "$POSTGRES_USER" -d "$POSTGRES_DB" >/dev/null 2>&1; then
        b_ok="$(http_get "http://127.0.0.1:${BACKEND_PORT}/health" 2>/dev/null || true)"
        p_ok="$(http_get "http://127.0.0.1:${APP_PORT}/api/health" 2>/dev/null || true)"
        if [ "$b_ok" = "OK" ] && [ "$p_ok" = "OK" ]; then
            break
        fi
    fi

    sleep 3
    elapsed=$((elapsed + 3))
done

if [ "$elapsed" -ge 60 ]; then
    run_compose ps || true
    fail "Os servicos nao atingiram estado saudavel em 60s apos o rollback."
fi

if [ -f "$CUR_FILE" ]; then
    cp "$CUR_FILE" "$PREV_FILE"
    chmod 0600 "$PREV_FILE"
fi

cat > "$CUR_FILE" <<EOF
GHCR_IMAGE_PREFIX=${TARGET_PREFIX}
IMAGE_TAG=${TARGET_TAG}
COMMIT_SHA=rollback
DEPLOYED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
EOF
chmod 0600 "$CUR_FILE"

printf '%s\t%s\trollback\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$TARGET_TAG" >> "$DEPLOY_PATH/.deploy-history.log"
chmod 0600 "$DEPLOY_PATH/.deploy-history.log"

log "Rollback concluido e verificado com sucesso! Versao ativa: ${TARGET_TAG}"
run_compose ps
