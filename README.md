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
export EVM_RPC_URL="ws://localhost:8546"
export HTTP_PORT=7777
export POSTGRES_USER=user
export POSTGRES_PASSWORD=password
export POSTGRES_DB=indexer
export DATABASE_URL="postgres://user:password@localhost:5432/indexer"
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

Before running the application, set the required environment variables:

```sh
export RPC_URL="ws://host.docker.internal:8546"
export HTTP_PORT=7777
export POSTGRES_USER=user
export POSTGRES_PASSWORD=password
export POSTGRES_DB=indexer
export DATABASE_URL="postgres://user:password@twine-db:5432/indexer"
```

**Note:** When using Docker, `host.docker.internal` allows the container to communicate with services running on the host machine.

### 2. Start Services with Docker Compose

Ensure you have Docker and Docker Compose installed. Also, make sure the execution client (e.g., Reth) is running. Then, start the indexer services:

```sh
docker compose up -d
```
---
