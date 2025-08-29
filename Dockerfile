FROM rust:1.86 AS base

#ARG GITHUB_USERNAME
#ARG GITHUB_TOKEN

RUN --mount=type=secret,id=github_token,env=GITHUB_TOKEN \
    --mount=type=secret,id=github_username,env=GITHUB_USERNAME \
    apt update && \
    apt install -y \
    build-essential \
    clang \
    libssl-dev \
    pkg-config && \
    rm -rf /var/lib/apt/lists/* && \
    git config --global credential.helper store && \
    echo "https://${GITHUB_USERNAME}:${GITHUB_TOKEN}@github.com" > ~/.git-credentials && \
    chmod 600 ~/.git-credentials

WORKDIR /app

COPY . .

RUN cargo build --release --bin api --bin indexer

FROM base AS dependency

RUN cargo install sea-orm-cli@1.1.7

FROM rust:1.85-alpine

RUN apk add musl-dev
WORKDIR /app

COPY --from=dependency /usr/local/cargo/bin/sea-orm-cli /usr/local/bin/sea-orm-cli
COPY --from=base /app/target/release/api /usr/local/bin/api
COPY --from=base /app/target/release/indexer /usr/local/bin/indexer
COPY --from=base /app/migration migration