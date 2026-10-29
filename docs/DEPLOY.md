# Guia de CI/CD e Deploy em Servidor Doméstico

Este documento descreve a arquitetura e a operação do pipeline de **Integração Contínua (CI)** e **Entrega Contínua (CD)** do **Personal Home Server Drive**, implementado em [`.github/workflows/ci-cd.yml`](../.github/workflows/ci-cd.yml).

---

## 1. Visão Geral da Arquitetura

```text
Push / PR (qualquer PR, main, develop)
        │
        ├──► [ci-backend]   cargo check --locked --all-targets + cargo test --locked
        └──► [ci-frontend]  npm ci + npm run build (tsc -b && vite build)
                  │
     (Apenas push/dispatch em refs/heads/main após aprovação do CI)
                  ▼
        [publish-images]
          • Calcula hash da árvore Git de Back-End/ e Front-End/
          • Reutiliza manifesto existente no GHCR se o serviço não mudou
          • Ou constrói e publica ghcr.io/<owner>/<repo>/{backend,frontend}:sha-<commit>
                  │
                  ▼
           [deploy]
          • Entra temporariamente na Tailnet como nó efêmero (tag:ci)
          • Valida chave de host SSH (StrictHostKeyChecking=yes)
          • Envia apenas arquivos de runtime para 100.69.7.71:22
          • Autentica no GHCR via stdin (config temporária em RAM/tmp)
          • Executa docker compose pull + up -d --no-build (sem tocar em dados/.env)
          • Verifica saúde (containers, Postgres, backend, frontend e proxy /api/health)
          • Reverte automaticamente para a tag anterior se a verificação falhar
```

### Arquivos envolvidos no repositório

| Arquivo | Função |
| --- | --- |
| `.github/workflows/ci-cd.yml` | Pipeline completo de CI, build/publicação no GHCR e deploy via Tailscale + SSH |
| `docker-compose.yml` | Definição base dos serviços (`postgres`, `backend`, `frontend`, `db-backup`), volumes e healthchecks |
| `docker-compose.prod.yml` | Override de produção que aponta `backend` e `frontend` para as imagens versionadas no GHCR |
| `scripts/deploy-remote.sh` | Script POSIX `sh` executado no servidor durante o deploy com pré-validações, healthchecks e auto-rollback |
| `scripts/rollback.sh` | Utilitário no servidor para reversão manual imediata para a release anterior ou um SHA específico |
| `scripts/db-backup.sh` | Rotina contínua de dump diário compactado do PostgreSQL (`.sql.gz`) |

---

## 2. Estratégia de Build Escolhida (Opção B — GHCR)

Optou-se por **compilar as imagens no GitHub Actions e publicá-las no GitHub Container Registry (GHCR)** pelos seguintes motivos técnicos:

1. **Alívio de CPU, RAM e disco no servidor doméstico:** o backend em Rust compila `axum`, `tokio`, `sqlx` (OpenSSL nativo) e `argon2`. Fazer `cargo build --release` na nuvem evita picos de CPU/memória no Ubuntu Server e elimina a necessidade de manter ~2–3 GB de imagens de build (`rust:1-bookworm`, `node:22-alpine` e cache `target/`) no servidor.
2. **Sem código-fonte nem chaves do GitHub no servidor:** o servidor recebe apenas os arquivos de orquestração (`docker-compose.yml`, `docker-compose.prod.yml`, `database.sql`, `scripts/*`) e baixa imagens enxutas (`debian:bookworm-slim` e `nginx:1.27-alpine`).
3. **Prevenção de builds desnecessários:** o job `publish-images` calcula `git rev-parse HEAD:Back-End` e `git rev-parse HEAD:Front-End`. Se o código de um serviço não mudou em relação a um deploy anterior, a imagem existente no GHCR (`tree-<hash>`) é apenas re-tagueada com `sha-<commit>` em ~1 segundo via `docker buildx imagetools create`, sem recompilar nem reiniciar o container inalterado no servidor.
4. **Rastreabilidade e rollback rápido:** toda publicação gera tags imutáveis `sha-<commit_completo>` (nunca dependendo apenas de `latest`), permitindo reversão em poucos segundos.
5. **Autenticação sem PAT adicional no pipeline:** tanto o push no Actions quanto o pull remoto durante o deploy utilizam o `GITHUB_TOKEN` efêmero do próprio workflow (transmitido por `stdin` sobre SSH e descartado ao fim da execução).

---

## 3. Fluxo de CI e Limitações Atuais de Testes

O CI roda automaticamente em **todos os Pull Requests** e nos **pushes para `main` e `develop`**, com permissão mínima (`contents: read`) e sem acesso a Secrets de produção:

- **Backend (`Back-End/`):** instala `pkg-config` e `libssl-dev`, executa `cargo check --locked --all-targets` e `cargo test --locked`.
- **Frontend (`Front-End/`):** configura Node.js 22 (`FROM node:22-alpine`), executa `npm ci` e `npm run build` (`tsc -b && vite build`).

> **Registro de limitação atual:** o backend possui atualmente **4 testes unitários/integração local** cobrindo validação de nomes seguros, deduplicação de extensão, inferência de MIME e proteção contra *path traversal* no adaptador de sistema de arquivos (`FsStore`). Ainda não existem testes automatizados para rotas HTTP/repositórios SQL nem suíte de testes/lint no frontend. Recomenda-se expandir essa cobertura gradualmente conforme novas funcionalidades forem adicionadas.

---

## 4. Integração com Tailscale e Análise da Política

O job `deploy` utiliza a Action oficial **`tailscale/github-action@v4`**:

```yaml
- uses: tailscale/github-action@v4
  with:
    oauth-client-id: ${{ secrets.TS_OAUTH_CLIENT_ID }}
    oauth-secret: ${{ secrets.TS_OAUTH_SECRET }}
    tags: tag:ci
```

### Compatibilidade da política atual da Tailnet

A configuração informada na sua Tailnet:

```json
{
  "tagOwners": {
    "tag:example": ["autogroup:admin"],
    "tag:ci": ["autogroup:admin"]
  },
  "grants": [
    {
      "src": ["tag:ci"],
      "dst": ["100.69.7.71"],
      "ip": ["tcp:22"]
    }
  ]
}
```

**É totalmente compatível com a implementação — nenhuma regra adicional precisa liberar o restante da rede doméstica:**

- `tagOwners` define `tag:ci`, permitindo que o OAuth Client provisione o runner com identidade `tag:ci`.
- `grants` libera **exclusivamente `tcp:22`** (SSH) da origem `tag:ci` para o IP Tailscale `100.69.7.71`, bloqueando qualquer outra porta ou dispositivo da rede local.
- **Verificação única no painel do Tailscale:** confirme em **Admin Console → Settings → OAuth clients / Trust credentials** que o OAuth Client criado possui o escopo **`Auth Keys` (`auth_keys`) com permissão `Write`** (e `Devices: Core` Write caso solicitado pelo painel) vinculado à tag **`tag:ci`**.
- **Dispositivos efêmeros:** ao autenticar via OAuth Client + `tags: tag:ci`, a `tailscale/github-action@v4` cria automaticamente um **nó efêmero** pré-aprovado e executa `tailscale logout` na etapa `post` ao final do job, removendo imediatamente o runner da Tailnet sem deixar máquinas órfãs.

---

## 5. Configuração Segura de SSH e Host Key

### 5.1. Gerar um par de chaves SSH exclusivo para o deploy

Em sua máquina local (fora do repositório!), gere um par Ed25519 dedicado e sem passphrase (necessário para execução não-interativa em `BatchMode=yes`):

```bash
ssh-keygen -t ed25519 -a 100 -C "github-actions-deploy@personal-drive" -f ~/.ssh/personal_drive_deploy -N ""
```

Isso criará:
- `~/.ssh/personal_drive_deploy` (**chave privada** → irá apenas para o Secret `SERVER_SSH_KEY` no GitHub; depois você pode apagá-la da máquina local se desejar).
- `~/.ssh/personal_drive_deploy.pub` (**chave pública** → será adicionada ao servidor).

### 5.2. Instalar a chave pública no servidor (preservando chaves existentes)

Conecte-se normalmente ao servidor Ubuntu (`ly_server_314151771415@100.69.7.71`) e verifique/adicione a chave **com `>>` (append)**, sem sobrescrever entradas existentes.

Recomenda-se restringir a chave no próprio OpenSSH a conexões vindas da faixa Tailscale (`100.64.0.0/10` e `fd7a:115c:a1e0::/48`) e desabilitar encaminhamentos (`restrict`):

```bash
# Na sua máquina local, exiba o conteúdo da chave pública:
cat ~/.ssh/personal_drive_deploy.pub
```

No servidor Ubuntu (logado como `ly_server_314151771415`):

```bash
mkdir -p ~/.ssh
chmod 700 ~/.ssh
touch ~/.ssh/authorized_keys
chmod 600 ~/.ssh/authorized_keys

# Adicione UMA nova linha ao final (substitua pelo conteúdo real do .pub):
echo 'restrict,from="100.64.0.0/10,fd7a:115c:a1e0::/48" ssh-ed25519 AAAA... github-actions-deploy@personal-drive' >> ~/.ssh/authorized_keys
```

### 5.3. Obter e verificar a chave de host do servidor (`SERVER_SSH_KNOWN_HOSTS`)

O pipeline utiliza **`StrictHostKeyChecking=yes`** e rejeita qualquer chave de host inesperada. Para obtê-la por um canal confiável:

1. **No próprio terminal do servidor Ubuntu** (`100.69.7.71`), leia a chave pública Ed25519 do daemon SSH e seu fingerprint:
   ```bash
   cat /etc/ssh/ssh_host_ed25519_key.pub
   ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
   ```
2. Monte a linha exata no formato `known_hosts` prefixando `100.69.7.71 ` antes de `ssh-ed25519 AAAA...` (sem o comentário final opcional, ou mantendo-o):
   ```bash
   # Execute no próprio servidor para gerar a linha pronta e verificada:
   printf '100.69.7.71 %s %s\n' "$(awk '{print $1}' /etc/ssh/ssh_host_ed25519_key.pub)" "$(awk '{print $2}' /etc/ssh/ssh_host_ed25519_key.pub)"
   ```
3. Copie essa linha (ex.: `100.69.7.71 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...`) para cadastrar no Secret `SERVER_SSH_KNOWN_HOSTS`.

### 5.4. Testar a autenticação SSH antes do primeiro deploy

Da sua máquina conectada à Tailscale:

```bash
printf '%s\n' '100.69.7.71 ssh-ed25519 AAAA...' > /tmp/test_known_hosts

ssh -i ~/.ssh/personal_drive_deploy \
  -o BatchMode=yes \
  -o IdentitiesOnly=yes \
  -o StrictHostKeyChecking=yes \
  -o UserKnownHostsFile=/tmp/test_known_hosts \
  ly_server_314151771415@100.69.7.71 \
  "id && docker compose version"

rm -f /tmp/test_known_hosts
```

---

## 6. Preparação do Servidor Ubuntu e Privilégios Docker

### 6.1. Aviso de segurança sobre privilégios Docker

Para atualizar containers sem prompt interativo de senha, o usuário `ly_server_314151771415` precisa acessar o Docker Engine.

> **Importante:** na arquitetura padrão do Docker Engine, pertencer ao grupo `docker` (acesso ao socket `/var/run/docker.sock`) **equivale na prática a privilégios de `root` no servidor**, pois permite montar caminhos arbitrários do host em containers. Por isso, a chave SSH de deploy deve ser exclusiva para esse fim, protegida com `restrict,from="100.64.0.0/10..."` em `authorized_keys`, e a Tailnet deve expor apenas `tcp:22` ao `tag:ci`.

Verifique no servidor se o usuário já acessa o Docker sem `sudo`:

```bash
# Logado como ly_server_314151771415:
docker info >/dev/null && docker compose version
```

Caso ainda não tenha acesso, adicione o usuário ao grupo `docker` (requer reconectar a sessão SSH para surtir efeito):

```bash
sudo usermod -aG docker ly_server_314151771415
```

### 6.2. Configurar o diretório de deploy e o arquivo `.env` no servidor

Por padrão, o pipeline utiliza `/home/ly_server_314151771415/Personal_Home_Server_Drive` (ou o caminho customizado na variável `SERVER_DEPLOY_PATH`).

**O pipeline nunca sobrescreve o arquivo `.env` do servidor** e **recusa o deploy se `POSTGRES_PASSWORD` ainda for `suasenha`**. Antes do primeiro deploy, crie o `.env` no servidor:

```bash
mkdir -p ~/Personal_Home_Server_Drive
chmod 700 ~/Personal_Home_Server_Drive

cat > ~/Personal_Home_Server_Drive/.env <<'EOF'
# Ajuste DATA_DIR para o seu HD/SSD dedicado se desejar (ex.: /mnt/hd-dados/personal-drive)
DATA_DIR=./data

PUID=1000
PGID=1000

APP_PORT=5174
BACKEND_PORT=5001
POSTGRES_PORT=5433

POSTGRES_USER=postgres
POSTGRES_PASSWORD=TROQUE_POR_UMA_SENHA_FORTE_AQUI
POSTGRES_DB=personal_drive

MAX_UPLOAD_MB=1024
TRASH_TTL_DAYS=30

BACKUP_INTERVAL_HOURS=24
BACKUP_KEEP_DAYS=14
EOF

chmod 600 ~/Personal_Home_Server_Drive/.env
```

*(Dica: confirme os valores de `PUID` e `PGID` rodando `id -u` e `id -g` no servidor Ubuntu para que os arquivos salvos em `storage/` pertençam exatamente ao seu usuário Linux.)*

---

## 7. Cadastro de Secrets e Variáveis no GitHub

No repositório do GitHub, acesse:
**`Settings` → `Secrets and variables` → `Actions`**

### 7.1. Aba `Secrets` (`New repository secret`)

| Nome do Secret | Obrigatório | Valor a cadastrar |
| --- | --- | --- |
| `TS_OAUTH_CLIENT_ID` | Sim | Client ID do OAuth Client criado no painel do Tailscale |
| `TS_OAUTH_SECRET` | Sim | Client Secret do OAuth Client criado no painel do Tailscale |
| `SERVER_SSH_KEY` | Sim | Conteúdo completo da chave privada (`~/.ssh/personal_drive_deploy`), incluindo `-----BEGIN OPENSSH PRIVATE KEY-----` até `-----END OPENSSH PRIVATE KEY-----` |
| `SERVER_SSH_KNOWN_HOSTS` | Sim | Linha verificada da chave pública de host do servidor (ex.: `100.69.7.71 ssh-ed25519 AAAA...`) |

*(Nenhum Secret adicional é exigido para o GHCR: o workflow usa o `GITHUB_TOKEN` automático.)*

### 7.2. Aba `Variables` (`New repository variable`)

| Nome da Variável | Obrigatório | Valor recomendado / Padrão |
| --- | --- | --- |
| `SERVER_HOST` | Sim | `100.69.7.71` |
| `SERVER_USER` | Sim | `ly_server_314151771415` |
| `SERVER_PORT` | Opcional | Padrão: `22` |
| `SERVER_DEPLOY_PATH` | Opcional | Padrão: `/home/ly_server_314151771415/Personal_Home_Server_Drive` |
| `IMAGE_PLATFORMS` | Opcional | Padrão: `linux/amd64` (altere para `linux/arm64` apenas se o servidor for ARM64) |

---

## 8. Executando o Primeiro Deploy e Acompanhando os Logs

1. Cadastre os 4 **Secrets** e as 2 **Variables** obrigatórias no GitHub e confirme que `~/Personal_Home_Server_Drive/.env` existe no servidor Ubuntu.
2. Envie/faça merge das alterações para a branch **`main`** (ou acesse a aba **Actions → CI/CD → Run workflow** selecionando a branch `main`).
3. Acompanhe na aba **Actions** do GitHub:
   - **`CI · Backend (Rust)`** e **`CI · Frontend (React / TypeScript)`** rodam em paralelo.
   - **`CD · Build e Publicação no GHCR`** publica `backend` e `frontend` tagueados com `sha-<commit>`.
   - **`CD · Deploy no Servidor Doméstico`** conecta na Tailnet, valida SSH, atualiza os containers e executa os healthchecks ponta a ponta.

### Consultar logs diretamente no servidor

```bash
cd ~/Personal_Home_Server_Drive

# Status e saúde dos 4 serviços (compatível com bash, sh e fish)
docker compose --env-file .env --env-file .release.env -f docker-compose.yml -f docker-compose.prod.yml ps

# Logs recentes
docker compose --env-file .env --env-file .release.env -f docker-compose.yml -f docker-compose.prod.yml logs -f --tail=100

# Histórico de releases aplicadas
cat .deploy-history.log
```

---

## 9. Persistência de Dados e Banco de Dados

O pipeline foi desenhado para **nunca executar comandos destrutivos** (`docker compose down -v`, `docker system prune -a --volumes`, `DROP TABLE`, etc.):

- **`${DATA_DIR}/storage/`**: pastas e arquivos reais dos usuários permanecem intactos no disco do servidor.
- **`${DATA_DIR}/pgdata/`**: arquivos internos do PostgreSQL 16 sobrevivem a todas as atualizações e reinicializações.
- **`${DATA_DIR}/backups/`**: dumps diários `.sql.gz` gerados pelo serviço `db-backup`.
- **`database.sql` vs `ensure_schema`**: o arquivo `database.sql` montado em `/docker-entrypoint-initdb.d/01-schema.sql:ro` é executado pelo container oficial do Postgres **apenas quando `pgdata` está vazio** (primeira inicialização). Em todos os boots subsequentes, o backend executa `ensure_schema` com instruções idempotentes (`CREATE TABLE IF NOT EXISTS`, `CREATE UNIQUE INDEX IF NOT EXISTS`). Caso no futuro haja uma migração de banco não-reversível, ela nunca deve ser automatizada sem backup prévio e validação manual.

---

## 10. Rollback e Recuperação

### 10.1. Rollback automático
Se durante um deploy qualquer container não atingir `healthy` ou os endpoints `/health`, `/` e `/api/health` não responderem em até 90 segundos, o `scripts/deploy-remote.sh` **restaura automaticamente a tag anterior** registrada em `.release.env` e encerra o job do GitHub Actions com erro para alertar você.

### 10.2. Rollback manual no servidor
A qualquer momento, conecte-se ao servidor via SSH e execute:

```bash
cd ~/Personal_Home_Server_Drive

# Reverter imediatamente para a versão anterior salva em .release.prev.env:
./scripts/rollback.sh

# Ou reverter para qualquer commit específico listado em .deploy-history.log:
./scripts/rollback.sh sha-<commit_completo>
```

> **Nota sobre imagens privadas em rollback manual:** como o `deploy-remote.sh` executa apenas `docker image prune -f` (removendo apenas imagens órfãs sem tag), a versão anterior permanece em cache local no servidor e o rollback é instantâneo mesmo offline. Caso precise baixar manualmente uma tag muito antiga de um pacote privado no GHCR fora do GitHub Actions, autentique antes com um Classic PAT de escopo `read:packages`:
> `docker login ghcr.io -u <seu-usuario-github>`

---

## 11. Diagnóstico de Falhas Comuns

| Sintoma no GitHub Actions | Causa provável | Como resolver |
| --- | --- | --- |
| `Configuração obrigatória ausente no GitHub Actions` | Algum Secret ou Variable não foi salvo no repositório | Verifique os nomes exatos na seção 7 (`Settings → Secrets and variables → Actions`) |
| `tailscale/github-action` falha com `403` / `unauthorized` | OAuth Client sem escopo `auth_keys` (Write) ou sem vínculo com `tag:ci` | Gere/ajuste o OAuth Client no Admin Console do Tailscale com `Auth Keys: Write` e tag `tag:ci` |
| `Host key verification failed` / `SERVER_SSH_KNOWN_HOSTS não contém entrada válida` | Valor de `SERVER_SSH_KNOWN_HOSTS` incompleto ou de outro IP | Gere novamente no servidor com `/etc/ssh/ssh_host_ed25519_key.pub` prefixado por `100.69.7.71 ` |
| `Permission denied (publickey)` no SSH | Chave pública ausente em `~/.ssh/authorized_keys` ou permissões incorretas | Confirme `chmod 700 ~/.ssh` e `chmod 600 ~/.ssh/authorized_keys` para `ly_server_314151771415` |
| `nao possui permissao para acessar o daemon Docker` | Usuário SSH fora do grupo `docker` | Rode `sudo usermod -aG docker ly_server_314151771415` no servidor |
| `POSTGRES_PASSWORD ... usa o valor de exemplo ('suasenha')` | `.env` no servidor não foi customizado | Edite `~/Personal_Home_Server_Drive/.env` e defina uma senha forte para `POSTGRES_PASSWORD` |
| `Existem alteracoes locais nao commitadas nos arquivos de deploy` | Arquivos gerenciados (`docker-compose.yml`, `scripts/`) foram editados manualmente num clone Git no servidor | Faça backup/commit/stash das edições manuais no servidor antes do próximo deploy |
