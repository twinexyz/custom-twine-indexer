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
# shared db config
export DATABASE_URL="postgresql://dbuser:password@localhost:5432/indexer"

# api specific
export API_PORT=7777

# deployed contracts 
export L1_MESSAGE_QUEUE_ADDRESS="0x610178dA211FEF7D417bC0e6FeD39F05609AD788"
export L1_ERC20_GATEWAY_ADDRESS="0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9"

export L2_TWINE_MESSENGER_ADDRESS="0xA51c1fc2f0D1a1b8494Ed1FE312d7C3a78Ed91C0"

export TOKENS_GATEWAY_PROGRAM_ADDRESS="BEdLPRG4d8TyY293gFuVkLE5zQ9qAeD1YWXpMkNyiYS"
export TWINE_CHAIN_PROGRAM_ADDRESS="8P6bCmFNhi3ZtTYRf4MwtsNkvV6NhtbVocQGFyymcSr5"

# chain specifics
export ETHEREUM__HTTP_RPC_URL="http://0.0.0.0:8570"
export TWINE__HTTP_RPC_URL="http://0.0.0.0:8545"
export SOLANA__HTTP_RPC_URL="http://0.0.0.0:8899"

export ETHEREUM__WS_RPC_URL="ws://0.0.0.0:8571"
export TWINE__WS_RPC_URL="ws://0.0.0.0:8546"
export SOLANA__WS_RPC_URL="ws://0.0.0.0:8590"

export ETHEREUM__CHAIN_ID=17000
export SOLANA__CHAIN_ID=900
export TWINE__CHAIN_ID=1337

export ETHEREUM__BLOCK_SYNC_BATCH_SIZE=10
export TWINE__BLOCK_SYNC_BATCH_SIZE=100
export SOLANA__BLOCK_SYNC_BATCH_SIZE=1000

export ETHEREUM__START_BLOCK=10
export TWINE__START_BLOCK=10
export SOLANA__START_BLOCK=10
``` 

### 2. Run the Indexer and API

Navigate to the project's root directory and start the API server:

```sh
cargo run --bin api --release
```

In a separate terminal, start the indexer:

```sh
cargo run --bin indexer --release
```

The indexer listens to an execution client instance (e.g., Reth). While running locally ensure that the execution client is running before starting the indexer.

---

## Running with Docker

### 1. Set Environment Variables

Before running the application, create a `.env` file with environment variables as (*please note the '__' used in some variables below*):

```sh
DATABASE_URL="postgresql://db_admin:dJXgaAqMM7PtrIJ@twine-db:5432/indexer_db"

API_PORT=7777

L1_MESSAGE_QUEUE_ADDRESS="0x610178dA211FEF7D417bC0e6FeD39F05609AD788"
L1_ERC20_GATEWAY_ADDRESS="0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9"

L2_TWINE_MESSENGER_ADDRESS="0xA51c1fc2f0D1a1b8494Ed1FE312d7C3a78Ed91C0"

TOKENS_GATEWAY_PROGRAM_ADDRESS="BEdLPRG4d8TyY293gFuVkLE5zQ9qAeD1YWXpMkNyiYS"
TWINE_CHAIN_PROGRAM_ADDRESS="8P6bCmFNhi3ZtTYRf4MwtsNkvV6NhtbVocQGFyymcSr5"

ETHEREUM__WS_RPC_URL="wss://rpc.ethereum.co"
TWINE__WS_RPC_URL="wss://rpc.twine.co"
SOLANA__WS_RPC_URL="wss://rpc.solana.co"

ETHEREUM__HTTP_RPC_URL="https://rpc.ethereum.co"
TWINE__HTTP_RPC_URL="https://rpc.twine.co"
SOLANA__HTTP_RPC_URL="https://rpc.solana.co"

ETHEREUM__CHAIN_ID=17000
SOLANA__CHAIN_ID=900
TWINE__CHAIN_ID=1337

ETHEREUM__BLOCK_SYNC_BATCH_SIZE=10
TWINE__BLOCK_SYNC_BATCH_SIZE=100
SOLANA__BLOCK_SYNC_BATCH_SIZE=1000

ETHEREUM__START_BLOCK=1000000
TWINE__START_BLOCK=0
SOLANA__START_BLOCK=1999223
```

### 2. Start Services with Docker Compose

Ensure you have Docker and Docker Compose installed. Also, make sure the execution client (e.g., Reth) is running. Then, start the indexer services:

```sh
docker compose up -d
```
---
