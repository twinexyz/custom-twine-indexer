FROM rust:1.89 AS base
ARG ARCH 

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

RUN cargo build --release --bin api --bin indexer && \
    cargo install sea-orm-cli --locked
    
RUN wget -c https://github.com/mikefarah/yq/releases/download/v4.45.1/yq_linux_${ARCH} -O /usr/bin/yq && \
    chmod +x /usr/bin/yq

FROM rust:1.89 as final

RUN apt update --allow-insecure-repositories && \
    apt install -y \
    build-essential \
    clang \
    libssl-dev \
    pkg-config && \
    rm -rf /var/lib/apt/lists/*

COPY --from=base /usr/local/cargo/bin/sea-orm-cli /usr/local/bin/sea-orm-cli
COPY --from=base /usr/bin/yq /usr/local/bin/yq

COPY --from=base /app/target/release/api /usr/local/bin/api
COPY --from=base /app/target/release/indexer /usr/local/bin/indexer
COPY --from=base /app/migration migration
