FROM rust:1.85-alpine AS base

RUN --mount=type=secret,id=github_token,env=GITHUB_TOKEN \
    --mount=type=secret,id=github_username,env=GITHUB_USERNAME \
    apk add --virtual .build-deps \
    pkgconf \
    openssl-dev \
    postgresql-dev \
    git \
    make \
    musl-dev \
    libcrypto3 \
    perl \
    openssl-libs-static && \
    # Set up Git credentials for private repositories
    git config --global credential.helper store && \
    echo "https://${GITHUB_USERNAME}:${GITHUB_TOKEN}@github.com" > ~/.git-credentials && \
    chmod 600 ~/.git-credentials

FROM base AS dependency

RUN cargo install sea-orm-cli@1.1.7

FROM base AS app

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

RUN  sed -i -E '/\[\[bin\]\]/{N;/name = "api"/{N;d}}; /name = "indexer"/{N;s/path = "[^"]+"/path = "dummy.rs"/}' Cargo.toml
RUN echo "fn main() {}" > dummy.rs


# Copy full source and build binaries
COPY . .
RUN cargo build --release --bin api --bin indexer

RUN rm -f ~/.git-credentials && \
    apk del .build-deps

#FROM gcr.io/distroless/cc-debian12
FROM rust:1.85-alpine

RUN apk add musl-dev
WORKDIR /app

COPY --from=dependency /usr/local/cargo/bin/sea-orm-cli /usr/local/bin/sea-orm-cli
COPY --from=app /app/target/release/api /usr/local/bin/api
COPY --from=app /app/target/release/indexer /usr/local/bin/indexer
COPY --from=app /app/migration migration
