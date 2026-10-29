#!/bin/sh
# Script executado remotamente no servidor doméstico pelo GitHub Actions (via SSH sobre Tailscale).
# Lê o token efêmero do GHCR via stdin (opcional se imagens públicas), valida pré-requisitos,
# atualiza apenas os arquivos de runtime sem tocar em .env ou dados persistentes, faz pull
# das imagens versionadas por SHA, atualiza os serviços e verifica a saúde ponta a ponta.

set -eu

# Lê token efêmero de leitura do GHCR via stdin sem ecoar nos logs
GHCR_TOKEN=""
if [ ! -t 0 ]; then
    IFS= read -r GHCR_TOKEN || true
fi

DEPLOY_PATH="${DEPLOY_PATH:?Variavel DEPLOY_PATH obrigatoria}"
BUNDLE_PATH="${BUNDLE_PATH:?Variavel BUNDLE_PATH obrigatoria}"
GHCR_USER="${GHCR_USER:-github-actions}"
GHCR_IMAGE_PREFIX="${GHCR_IMAGE_PREFIX:?Variavel GHCR_IMAGE_PREFIX obrigatoria}"
IMAGE_TAG="${IMAGE_TAG:?Variavel IMAGE_TAG obrigatoria}"
COMMIT_SHA="${COMMIT_SHA:-unknown}"

umask 077
TMP_WORK="$(mktemp -d /tmp/pd-work.XXXXXX)"
DOCKER_CFG="${TMP_WORK}/docker-config"
mkdir -p "$DOCKER_CFG"

cleanup() {
    rm -rf "$TMP_WORK"
    if [ -n "${BUNDLE_PATH:-}" ] && [ -f "$BUNDLE_PATH" ]; then
        rm -f "$BUNDLE_PATH"
    fi
}
trap cleanup EXIT INT TERM

log() {
    printf '[deploy %s] %s\n' "$(date -u +%H:%M:%S)" "$*"
}

fail() {
    printf '[deploy ERRO] %s\n' "$*" >&2
    exit 1
}

# 1. Verifica acesso ao Docker e Docker Compose sem conceder privilégios automaticamente
if ! command -v docker >/dev/null 2>&1; then
    fail "Docker nao encontrado no servidor. Instale o Docker Engine e o plugin Docker Compose antes do deploy."
fi

USE_SUDO=0
if docker info >/dev/null 2>&1; then
    USE_SUDO=0
elif command -v sudo >/dev/null 2>&1 && sudo -n docker info >/dev/null 2>&1; then
    USE_SUDO=1
else
    fail "O usuario '$(id -un)' nao possui permissao para acessar o daemon Docker sem senha interativa. Adicione o usuario ao grupo 'docker' (ou configure permissao restrita conforme docs/DEPLOY.md)."
fi

run_docker() {
    if [ "$USE_SUDO" -eq 1 ]; then
        sudo -n docker --config "$DOCKER_CFG" "$@"
    else
        docker --config "$DOCKER_CFG" "$@"
    fi
}

if ! run_docker compose version >/dev/null 2>&1; then
    fail "Plugin 'docker compose' (v2+) nao esta disponivel no servidor."
fi

# Helper HTTP local (suporta curl ou wget sem expor dados sensíveis)
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

# Extrai chave simples de arquivo .env sem executá-lo como shell script
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

# 2. Protege alterações locais caso DEPLOY_PATH seja um checkout Git existente
mkdir -p "$DEPLOY_PATH"
if [ -d "$DEPLOY_PATH/.git" ] && command -v git >/dev/null 2>&1; then
    dirty="$(git -C "$DEPLOY_PATH" status --porcelain -- \
        docker-compose.yml docker-compose.prod.yml database.sql scripts/ 2>/dev/null || true)"
    if [ -n "$dirty" ]; then
        printf '%s\n' "$dirty" >&2
        fail "Existem alteracoes locais nao commitadas nos arquivos de deploy em '$DEPLOY_PATH'. Revise-as antes de prosseguir."
    fi
fi

# 3. Extrai apenas arquivos de runtime (nunca sobrescreve .env nem dados persistentes)
if [ ! -f "$BUNDLE_PATH" ]; then
    fail "Pacote de deploy '$BUNDLE_PATH' nao encontrado."
fi

log "Atualizando arquivos de runtime em ${DEPLOY_PATH}..."
STAGE_DIR="${TMP_WORK}/bundle"
mkdir -p "$STAGE_DIR"
tar -xzf "$BUNDLE_PATH" -C "$STAGE_DIR"

mkdir -p "$DEPLOY_PATH/scripts" "$DEPLOY_PATH/Back-End" "$DEPLOY_PATH/Front-End"
cp "$STAGE_DIR/docker-compose.yml" "$DEPLOY_PATH/docker-compose.yml"
cp "$STAGE_DIR/docker-compose.prod.yml" "$DEPLOY_PATH/docker-compose.prod.yml"
cp "$STAGE_DIR/database.sql" "$DEPLOY_PATH/database.sql"
cp "$STAGE_DIR/.env.example" "$DEPLOY_PATH/.env.example"
cp "$STAGE_DIR/scripts/db-backup.sh" "$DEPLOY_PATH/scripts/db-backup.sh"
cp "$STAGE_DIR/scripts/deploy-remote.sh" "$DEPLOY_PATH/scripts/deploy-remote.sh"
cp "$STAGE_DIR/scripts/rollback.sh" "$DEPLOY_PATH/scripts/rollback.sh"

# Permissões legíveis pelos containers (ex.: postgres uid 999 lê 01-schema.sql)
chmod 0644 \
    "$DEPLOY_PATH/docker-compose.yml" \
    "$DEPLOY_PATH/docker-compose.prod.yml" \
    "$DEPLOY_PATH/database.sql" \
    "$DEPLOY_PATH/.env.example"
chmod 0755 \
    "$DEPLOY_PATH/scripts/db-backup.sh" \
    "$DEPLOY_PATH/scripts/deploy-remote.sh" \
    "$DEPLOY_PATH/scripts/rollback.sh"

# 4. Valida .env de produção e diretórios persistentes no servidor
ENV_FILE="$DEPLOY_PATH/.env"
if [ ! -f "$ENV_FILE" ]; then
    fail "Arquivo '$ENV_FILE' ausente no servidor. Crie-o a partir de '$DEPLOY_PATH/.env.example' configurando POSTGRES_PASSWORD forte e DATA_DIR antes do primeiro deploy."
fi

PG_PASS="$(read_env_key POSTGRES_PASSWORD "$ENV_FILE" "")"
if [ -z "$PG_PASS" ] || [ "$PG_PASS" = "suasenha" ]; then
    unset PG_PASS
    fail "POSTGRES_PASSWORD em '$ENV_FILE' esta vazio ou ainda usa o valor de exemplo ('suasenha'). Defina uma senha forte antes de continuar."
fi
unset PG_PASS

DATA_DIR_RAW="$(read_env_key DATA_DIR "$ENV_FILE" "./data")"
case "$DATA_DIR_RAW" in
    /*) DATA_DIR_ABS="$DATA_DIR_RAW" ;;
    *)  DATA_DIR_ABS="$DEPLOY_PATH/${DATA_DIR_RAW#./}" ;;
esac
mkdir -p "$DATA_DIR_ABS/pgdata" "$DATA_DIR_ABS/storage" "$DATA_DIR_ABS/backups"

APP_PORT="$(read_env_key APP_PORT "$ENV_FILE" "5174")"
BACKEND_PORT="$(read_env_key BACKEND_PORT "$ENV_FILE" "5001")"
POSTGRES_USER="$(read_env_key POSTGRES_USER "$ENV_FILE" "postgres")"
POSTGRES_DB="$(read_env_key POSTGRES_DB "$ENV_FILE" "personal_drive")"

# Lê release anterior (para rollback automático em caso de falha nos healthchecks)
PREV_IMAGE_PREFIX=""
PREV_IMAGE_TAG=""
if [ -f "$DEPLOY_PATH/.release.env" ]; then
    PREV_IMAGE_PREFIX="$(read_env_key GHCR_IMAGE_PREFIX "$DEPLOY_PATH/.release.env" "")"
    PREV_IMAGE_TAG="$(read_env_key IMAGE_TAG "$DEPLOY_PATH/.release.env" "")"
fi

build_compose_env() {
    target_prefix="$1"
    target_tag="$2"
    out_file="$3"
    cat "$ENV_FILE" > "$out_file"
    printf '\nGHCR_IMAGE_PREFIX=%s\nIMAGE_TAG=%s\n' "$target_prefix" "$target_tag" >> "$out_file"
    chmod 0600 "$out_file"
}

COMPOSE_ENV="${TMP_WORK}/compose.env"
build_compose_env "$GHCR_IMAGE_PREFIX" "$IMAGE_TAG" "$COMPOSE_ENV"

run_compose() {
    run_docker compose \
        --project-directory "$DEPLOY_PATH" \
        --env-file "$COMPOSE_ENV" \
        -f "$DEPLOY_PATH/docker-compose.yml" \
        -f "$DEPLOY_PATH/docker-compose.prod.yml" \
        "$@"
}

# 5. Autentica temporariamente no GHCR e baixa as imagens da revisão alvo
if [ -n "$GHCR_TOKEN" ]; then
    log "Autenticando temporariamente no ghcr.io..."
    printf '%s' "$GHCR_TOKEN" | run_docker login ghcr.io -u "$GHCR_USER" --password-stdin >/dev/null
    unset GHCR_TOKEN
fi

log "Validando configuracao final do Docker Compose..."
run_compose config -q

log "Baixando imagens ${GHCR_IMAGE_PREFIX}/{backend,frontend}:${IMAGE_TAG}..."
run_compose pull backend frontend

# Verifica compatibilidade de arquitetura antes de recriar containers
HOST_ARCH="$(uname -m)"
case "$HOST_ARCH" in
    x86_64)  EXPECTED_ARCH="amd64" ;;
    aarch64) EXPECTED_ARCH="arm64" ;;
    armv7l)  EXPECTED_ARCH="arm" ;;
    *)       EXPECTED_ARCH="$HOST_ARCH" ;;
esac

for svc_img in "${GHCR_IMAGE_PREFIX}/backend:${IMAGE_TAG}" "${GHCR_IMAGE_PREFIX}/frontend:${IMAGE_TAG}"; do
    IMG_ARCH="$(run_docker image inspect --format '{{.Architecture}}' "$svc_img")"
    if [ "$IMG_ARCH" != "$EXPECTED_ARCH" ]; then
        fail "Arquitetura da imagem '$svc_img' ($IMG_ARCH) difere da arquitetura do servidor ($EXPECTED_ARCH / $HOST_ARCH)."
    fi
done

# 6. Atualiza os serviços preservando volumes persistentes (sem down -v)
log "Aplicando atualizacao dos containers..."
run_compose up -d --no-build --remove-orphans

verify_health() {
    timeout_secs="${1:-90}"
    elapsed=0
    step_secs=3

    while [ "$elapsed" -lt "$timeout_secs" ]; do
        all_ok=1

        for cname in personal-drive-postgres personal-drive-backend personal-drive-frontend; do
            state="$(run_docker inspect --format '{{.State.Status}}:{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$cname" 2>/dev/null || echo 'missing:none')"
            if [ "$state" != "running:healthy" ]; then
                all_ok=0
                break
            fi
        done

        bkp_state="$(run_docker inspect --format '{{.State.Status}}:{{.State.Restarting}}' personal-drive-db-backup 2>/dev/null || echo 'missing:true')"
        if [ "$bkp_state" != "running:false" ]; then
            all_ok=0
        fi

        if [ "$all_ok" -eq 1 ]; then
            # Valida banco, backend direto, frontend e proxy frontend -> backend (/api/health)
            if run_docker exec personal-drive-postgres pg_isready -U "$POSTGRES_USER" -d "$POSTGRES_DB" >/dev/null 2>&1; then
                backend_resp="$(http_get "http://127.0.0.1:${BACKEND_PORT}/health" 2>/dev/null || true)"
                frontend_resp="$(http_get "http://127.0.0.1:${APP_PORT}/" 2>/dev/null || true)"
                proxy_resp="$(http_get "http://127.0.0.1:${APP_PORT}/api/health" 2>/dev/null || true)"

                if [ "$backend_resp" = "OK" ] && [ "$proxy_resp" = "OK" ] && printf '%s' "$frontend_resp" | grep -q 'id="root"'; then
                    return 0
                fi
            fi
        fi

        sleep "$step_secs"
        elapsed=$((elapsed + step_secs))
    done

    return 1
}

dump_safe_diagnostics() {
    log "Estado atual dos containers:"
    run_compose ps || true
    for cname in personal-drive-postgres personal-drive-backend personal-drive-frontend personal-drive-db-backup; do
        printf '\n--- Ultimas linhas de log: %s ---\n' "$cname" >&2
        run_docker logs --tail 25 "$cname" 2>&1 | sed -E 's#(postgres(ql)?://[^:/@]+:)[^@]+@#\1***@#g' >&2 || true
    done
}

log "Verificando saude dos containers, PostgreSQL, backend, frontend e proxy /api/health..."
if ! verify_health 90; then
    dump_safe_diagnostics

    if [ -n "$PREV_IMAGE_PREFIX" ] && [ -n "$PREV_IMAGE_TAG" ] && [ "$PREV_IMAGE_TAG" != "$IMAGE_TAG" ]; then
        log "Falha de saude detectada. Iniciando rollback automatico para ${PREV_IMAGE_TAG}..."
        build_compose_env "$PREV_IMAGE_PREFIX" "$PREV_IMAGE_TAG" "$COMPOSE_ENV"
        if run_compose up -d --no-build --remove-orphans && verify_health 60; then
            log "Rollback automatico concluido: servicos restaurados para ${PREV_IMAGE_TAG}."
        else
            log "ATENCAO: rollback automatico para ${PREV_IMAGE_TAG} tambem reportou falha. Verifique o servidor."
        fi
    fi

    fail "Deploy da versao ${IMAGE_TAG} reprovado na verificacao de saude."
fi

# 7. Persiste metadados da release ativa/anterior e limpa apenas camadas órfãs (<none>)
if [ -n "$PREV_IMAGE_PREFIX" ] && [ -n "$PREV_IMAGE_TAG" ] && [ "$PREV_IMAGE_TAG" != "$IMAGE_TAG" ]; then
    cp "$DEPLOY_PATH/.release.env" "$DEPLOY_PATH/.release.prev.env"
    chmod 0600 "$DEPLOY_PATH/.release.prev.env"
fi

cat > "$DEPLOY_PATH/.release.env" <<EOF
GHCR_IMAGE_PREFIX=${GHCR_IMAGE_PREFIX}
IMAGE_TAG=${IMAGE_TAG}
COMMIT_SHA=${COMMIT_SHA}
DEPLOYED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
EOF
chmod 0600 "$DEPLOY_PATH/.release.env"

printf '%s\t%s\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$IMAGE_TAG" "$COMMIT_SHA" >> "$DEPLOY_PATH/.deploy-history.log"
chmod 0600 "$DEPLOY_PATH/.deploy-history.log"

run_docker image prune -f >/dev/null 2>&1 || true

log "Deploy concluido e validado com sucesso! Versao ativa: ${IMAGE_TAG}"
run_compose ps
