# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

# Dependências ficam nesta camada. O registro do Cargo fica em cache do BuildKit
# para um rebuild no servidor não baixar os crates de novo.
COPY Cargo.toml Cargo.lock ./
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    mkdir -p src \
    && echo "pub fn dummy() {}" > src/lib.rs \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src target/release/.fingerprint/drive-* target/release/deps/*drive-*

# O passo acima deixa um main vazio. Arquivos copiados podem ter mtime anterior
# a esse binário; o touch força o Cargo a compilar o servidor de verdade.
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    find src -type f -exec touch {} + \
    && cargo build --release --locked \
    && grep -a -q "Servidor inicializado" target/release/drive

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 gosu curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/drive /usr/local/bin/drive
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

EXPOSE 5000

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["serve"]
