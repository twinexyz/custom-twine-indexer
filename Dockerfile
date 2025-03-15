FROM rust:1.81-alpine AS builder

ARG GITHUB_TOKEN
ARG GITHUB_USERNAME

RUN apk add --no-cache \
    pkgconf \
    openssl-dev \
    postgresql-dev \
    git \
    musl-dev \
    libcrypto3 \
    openssl-libs-static

#RUN cargo install sea-orm-cli

WORKDIR /app

COPY Cargo.* ./

RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

COPY . .

RUN git config --global credential.helper store && \
    echo "https://${GITHUB_USERNAME}:${GITHUB_TOKEN}@github.com" > ~/.git-credentials && \
    chmod 600 ~/.git-credentials && \
    rm -rf ~/.git-credentials

RUN cargo build --release --bin api --bin indexer

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /usr/local/cargo/bin/sea-orm-cli /usr/local/bin/sea-orm-cli
COPY --from=builder /app/target/release/api /usr/local/bin/api
COPY --from=builder /app/target/release/indexer /usr/local/bin/indexer
COPY --from=builder /app/migration /app/migration
