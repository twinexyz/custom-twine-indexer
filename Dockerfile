FROM rust:1.81-alpine AS builder

ARG GITHUB_TOKEN
ARG GITHUB_USERNAME

#RUN apk add \
#    pkgconf \
#    openssl-dev \
#    postgresql-dev \
#    git \
#    musl-dev \
#    libcrypto3 \
#    openssl-libs-static
# Install system dependencies in a single layer
RUN apk add --no-cache --virtual .build-deps \
    pkgconf \
    openssl-dev \
    postgresql-dev \
    git \
    musl-dev \
    libcrypto3 \
    openssl-libs-static && \
    # Set up Git credentials for private repositories
    git config --global credential.helper store && \
    echo "https://${GITHUB_USERNAME}:${GITHUB_TOKEN}@github.com" > ~/.git-credentials && \
    chmod 600 ~/.git-credentials

WORKDIR /app

# Copy dependency files and bin/ directory
COPY Cargo.toml Cargo.lock ./

RUN  sed -i -E '/\[\[bin\]\]/{N;/name = "api"/{N;d}}; /name = "indexer"/{N;s/path = "[^"]+"/path = "dummy.rs"/}' Cargo.toml
RUN echo "fn main() {}" > dummy.rs

RUN cargo build --release

# Copy full source and build binaries
COPY . .

RUN cargo build --release --bin api --bin indexer

RUN rm -f ~/.git-credentials && \
    apk del .build-deps

FROM gcr.io/distroless/cc-debian12

COPY --from=builder /usr/local/cargo/bin/sea-orm-cli /usr/local/bin/sea-orm-cli
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/api /usr/local/bin/api
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/indexer /usr/local/bin/indexer
COPY --from=builder /app/migration /app/migration
