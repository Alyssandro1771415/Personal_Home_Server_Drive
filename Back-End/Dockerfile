FROM rust:1-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Cache de dependências
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && echo "pub fn dummy() {}" > src/lib.rs \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src target/release/.fingerprint/drive-* target/release/deps/drive* target/release/deps/libdrive*

# Compilação real da aplicação
COPY src ./src
RUN cargo build --release --locked

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
