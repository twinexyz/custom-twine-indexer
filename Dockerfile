FROM rust:1.81-slim AS builder

RUN apt update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev

RUN cargo install sea-orm-cli

WORKDIR /app

#COPY Cargo.toml Cargo.lock ./
#COPY src ./src
#COPY bin ./bin
#COPY migration ./migration
#
COPY . .

RUN git config --global credential.helper store && \
    echo "https://github.com/" > ~/.git-credentials && \
    chmod 600 ~/.git-credentials

RUN cargo build --release --bin api --bin indexer

FROM debian:bookworm-slim

FROM rust:1.81

RUN apt update && \
    apt install -y \
    libssl-dev \
    libpq-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/local/cargo/bin/sea-orm-cli /usr/local/bin/sea-orm-cli
COPY --from=builder /app/target/release/api /usr/local/bin/api
COPY --from=builder /app/target/release/indexer /usr/local/bin/indexer
COPY --from=builder /app/migration /app/migration
