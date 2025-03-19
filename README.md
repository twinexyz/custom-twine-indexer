[![Build and Deploy](https://github.com/twinexyz/custom-twine-indexer/actions/workflows/build_and_deploy.yml/badge.svg?branch=main)](https://github.com/twinexyz/custom-twine-indexer/actions/workflows/build_and_deploy.yml)
[![Secrets and code scan](https://github.com/twinexyz/custom-twine-indexer/actions/workflows/scan.yml/badge.svg)](https://github.com/twinexyz/custom-twine-indexer/actions/workflows/scan.yml)
[![Security Audit](https://github.com/twinexyz/custom-twine-indexer/actions/workflows/cargo-audit.yml/badge.svg)](https://github.com/twinexyz/custom-twine-indexer/actions/workflows/cargo-audit.yml)
[![Test](https://github.com/twinexyz/custom-twine-indexer/actions/workflows/test.yml/badge.svg)](https://github.com/twinexyz/custom-twine-indexer/actions/workflows/test.yml)




# Twine Indexer Setup Guide

Twine Indexer is a Rust-based indexing service that listens to an Ethereum execution client (e.g., Reth) and indexes relevant blockchain data into a PostgreSQL database.

## Prerequisites

Before running the indexer, ensure you have:
- A local instance of `cargo` and `rustc` installed.
- A running execution client (e.g., Reth).
- PostgreSQL installed and running.
- Docker and Docker Compose installed (if using the Docker setup).

---

## Running with Cargo

### 1. Set Environment Variables

Export the required environment variables:

```sh
export EVM_RPC_URL="ws://0.0.0.0:8571"
export TWINE_RPC_URL="ws://0.0.0.0:8546"
export HTTP_PORT=7777
export POSTGRES_USER=nobel
export POSTGRES_PASSWORD=password
export POSTGRES_DB=indexer
export DATABASE_URL="postgresql://nobel_db:nobel@localhost:5432/twine_l2"
export SOLANA_RPC_URL="http://127.0.0.1:8899"
export SOLANA_WS_URL="ws://127.0.0.1:8900"
export TWINE_CHAIN_PROGRAM_ID="8P6bCmFNhi3ZtTYRf4MwtsNkvV6NhtbVocQGFyymcSr5"
export TOKENS_GATEWAY_PROGRAM_ID="BEdLPRG4d8TyY293gFuVkLE5zQ9qAeD1YWXpMkNyiYS"
export EVM_CHAIN_ID=17000
export SOLANA_CHAIN_ID=900
export TWINE_CHAIN_ID=1337
export L1_MESSAGE_QUEUE_ADDRESS="0x1234567890abcdef1234567890abcdef12345678"
export L2_TWINE_MESSENGER_ADDRESS="0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef"
export L1_TWINE_MESSENGER_ADDRESS="0xabcdef1234567890abcdef1234567890abcdef12"
export L1_ERC20_GATEWAY_ADDRESS="0x7890abcdef1234567890abcdef1234567890abcd"
``` 

**Note:** Ensure that `RPC_URL` is a WebSocket (`ws://`) URL, as the indexer requires WebSocket communication.

### 2. Run the Indexer and API

Navigate to the project's root directory and start the API server:

```sh
cargo run --bin api --release
```

In a separate terminal, start the indexer:

```sh
cargo run --bin indexer --release
```

The indexer listens to an execution client instance (e.g., Reth). Ensure that the execution client is running before starting the indexer.

---

## Running with Docker

### 1. Set Environment Variables

Before running the application, create a `.env` file with environment variables as:

```sh
EVM_RPC_URL="ws://host.docker.internal:8571"
TWINE_RPC_URL="ws://host.docker.internal:8546"
SOLANA_RPC_URL="http://host.docker.internal:8899"|
SOLANA_WS_URL="ws://127.0.0.1:8900"
HTTP_PORT=7777
POSTGRES_USER=user
POSTGRES_PASSWORD=password
POSTGRES_DB=indexer
DATABASE_URL="postgres://user:password@twine-db:5432/indexer"
TWINE_CHAIN_PROGRAM_ID="0x1234567890abcdef1234567890abcdef12345678"
TOKENS_GATEWAY_PROGRAM_ID="0x1234567890abcdef1234567890abcdef12345678"
EVM_CHAIN_ID=17000
SOLANA_CHAIN_ID=900
TWINE_CHAIN_ID=1337
L1_MESSAGE_QUEUE_ADDRESS="0x1234567890abcdef1234567890abcdef12345678"
L2_TWINE_MESSENGER_ADDRESS="0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef"
L1_TWINE_MESSENGER_ADDRESS="0xabcdef1234567890abcdef1234567890abcdef12"
L1_ERC20_GATEWAY_ADDRESS="0x7890abcdef1234567890abcdef1234567890abcd"
```

**Note:** When using Docker, `host.docker.internal` allows the container to communicate with services running on the host machine.

### 2. Start Services with Docker Compose

Ensure you have Docker and Docker Compose installed. Also, make sure the execution client (e.g., Reth) is running. Then, start the indexer services:

```sh
docker compose up -d
```
---


