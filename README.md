# Personal Home Server Drive

Drive pessoal para servidor caseiro rodando via **Docker Compose** (PostgreSQL + Back-End Rust/Axum + Front-End React/Nginx + rotina automática de backup do banco).

Os arquivos e pastas são gravados **diretamente no disco do servidor sem criptografia proprietária da aplicação**, mantendo exatamente a mesma hierarquia de pastas e nomes reais criados na interface web.

## Subindo toda a aplicação

1. *(Opcional)* Copie `.env.example` para `.env` na raiz e ajuste `DATA_DIR` (por exemplo, apontando para um HD/SSD dedicado como `/mnt/hd-dados/personal-drive`), `POSTGRES_PASSWORD` e `APP_PORT`:
   ```bash
   cp .env.example .env
   ```
2. Suba todos os serviços com build:
   ```bash
   docker compose up -d --build
   ```
3. Acesse no navegador (ou por qualquer dispositivo na rede local / Tailscale):
   - **Aplicação Web:** `http://localhost:5174` (ou `http://<IP-DO-SERVIDOR>:5174`)

## Onde ficam meus dados no disco?

Por padrão (`DATA_DIR=./data`), tudo fica persistido fora dos containers em três pastas no host:

- `./data/storage/<seu-usuario>/`: **suas pastas e arquivos reais**, legíveis diretamente pelo Linux, Samba, SSH ou plugando o HD em qualquer outro computador. Itens enviados à lixeira ficam temporariamente em `./data/storage/<seu-usuario>/.lixeira/` até expirarem.
- `./data/pgdata/`: dados internos do PostgreSQL.
- `./data/backups/`: dumps diários compactados (`.sql.gz`) gerados automaticamente pelo container `db-backup`.

> **Importante:** Rebuildar (`docker compose up -d --build`) ou reiniciar containers **não apaga seus dados**. Para proteção contra falha física de HD/SSD, sincronize periodicamente a pasta `DATA_DIR` para um segundo disco ou HD externo (ex.: `rsync -avh ./data/ /mnt/hd-externo/personal-drive-backup/`).

## Comandos úteis de emergência / manutenção

### Esqueci minha senha
Como os arquivos ficam abertos no disco, redefinir a senha não afeta nenhum arquivo:
```bash
docker compose exec backend drive reset-password <seu-usuario> <nova-senha>
```

### Sincronizar arquivos copiados direto no disco (ou recuperar após recriar o banco)
Se você copiar pastas/arquivos diretamente para `./data/storage/<seu-usuario>/` pelo Linux, ou se o banco PostgreSQL precisar ser recriado do zero (basta cadastrar o mesmo nome de usuário novamente ou rodar o comando abaixo):
```bash
docker compose exec -u 1000:1000 backend drive sync-disk
```

## CI/CD e Deploy Automatizado

O projeto conta com pipeline de CI/CD via GitHub Actions, GHCR, Tailscale e SSH. Consulte o guia completo de configuração, segurança e rollback em [`docs/DEPLOY.md`](docs/DEPLOY.md).
