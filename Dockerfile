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

WORKDIR /app

# Copy dependency files and bin/ directory
COPY Cargo.toml Cargo.lock ./
COPY bin bin

RUN if [ -n "$GITHUB_TOKEN" ] && [ -n "$GITHUB_USERNAME" ]; then \
        git config --global credential.helper store && \
        echo "https://${GITHUB_USERNAME}:${GITHUB_TOKEN}@github.com" > ~/.git-credentials && \
        chmod 600 ~/.git-credentials; \
    fi && \
    mkdir src && echo "pub mod dummy;" > src/lib.rs && \
    cargo build --release --target x86_64-unknown-linux-musl --lib && \
    rm -rf src ~/.git-credentials

# Copy full source and build binaries
COPY . .
RUN if [ -n "$GITHUB_TOKEN" ] && [ -n "$GITHUB_USERNAME" ]; then \
        git config --global credential.helper store && \
        echo "https://${GITHUB_USERNAME}:${GITHUB_TOKEN}@github.com" > ~/.git-credentials && \
        chmod 600 ~/.git-credentials; \
    fi && \
    cargo build --release --target x86_64-unknown-linux-musl --bin api --bin indexer && \
    rm -f ~/.git-credentials

FROM gcr.io/distroless/cc-debian12

COPY --from=builder /usr/local/cargo/bin/sea-orm-cli /usr/local/bin/sea-orm-cli
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/api /usr/local/bin/api
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/indexer /usr/local/bin/indexer
COPY --from=builder /app/migration /app/migration
