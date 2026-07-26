# TPT Oikos — Deployment Guide

This guide covers deploying a TPT Oikos node, from single-node testnet to multi-node network.

## Quick Deploy (Single Node)

### 1. Build the node

```bash
cargo build --release -p koinon-node
```

### 2. Create configuration

```bash
mkdir -p ~/.tpt-oikos
cat > ~/.tpt-oikos/config.toml << 'EOF'
[data_dir]
path = "/home/user/.tpt-oikos/data"

[rpc]
port = 8333

[block]
time_ms = 1000

[log]
level = "info"

[genesis]
chain_id = 1
treasury_balance = "300000000000000000000000000"
EOF
```

### 3. Start the node

```bash
# Start with default config
./target/release/koinon-node

# Start with custom config
./target/release/koinon-node --config ~/.tpt-oikos/config.toml

# Start with debug logging
RUST_LOG=debug ./target/release/koinon-node
```

### 4. Verify it's running

```bash
# Health check
curl http://localhost:8333/health

# Chain status
curl http://localhost:8333/status

# List validators
curl http://localhost:8333/validators
```

## Multi-Node Network

### 1. Start seed node

```bash
# Node 1 (seed)
./target/release/koinon-node --config node1.toml
```

### 2. Start peer nodes

```bash
# Node 2 (connects to seed)
./target/release/koinon-node --config node2.toml --seed 127.0.0.1:8333

# Node 3 (connects to seed)
./target/release/koinon-node --config node3.toml --seed 127.0.0.1:8333
```

### 3. Verify network

```bash
# Check peer count on each node
curl http://localhost:8333/peers

# Check consensus height
curl http://localhost:8333/status
```

## Configuration Reference

```toml
# Database storage path
[data_dir]
path = "/var/lib/tpt-oikos"

# RPC API server
[rpc]
port = 8333
bind = "0.0.0.0"

# Block production
[block]
time_ms = 1000          # Block time in milliseconds
max_tx_per_block = 1000  # Maximum transactions per block

# P2P networking
[network]
bind = "0.0.0.0:8334"   # P2P listen address
seed_nodes = []          # Seed nodes to connect to
max_peers = 50           # Maximum peer connections
peer_timeout_secs = 30   # Peer timeout in seconds

# Logging
[log]
level = "info"           # trace, debug, info, warn, error
format = "pretty"        # pretty, compact, json

# Genesis configuration
[genesis]
chain_id = 1             # Unique chain identifier
treasury_balance = "300000000000000000000000000"  # Initial treasury (300M OIKOS in base units)
```

## Node Operations

### Register a validator

```bash
curl -X POST http://localhost:8333/validators \
  -H "Content-Type: application/json" \
  -d '{"operator_did": "did:example:operator1"}'
```

### Stake tokens

```bash
curl -X POST http://localhost:8333/stake \
  -H "Content-Type: application/json" \
  -d '{"validator_id": 1, "amount": "100000000000000000000000"}'
```

### Submit a transaction

```bash
curl -X POST http://localhost:8333/tx \
  -H "Content-Type: application/json" \
  -d '{
    "kind": "TransferKoin",
    "sender": 1,
    "recipient": 2,
    "koin_amount": "1000000000000000000"
  }'
```

### Query account balance

```bash
curl http://localhost:8333/accounts/1
```

### Create treasury proposal

```bash
curl -X POST http://localhost:8333/propose \
  -H "Content-Type: application/json" \
  -d '{
    "proposer": "did:example:proposer",
    "recipient": "did:example:recipient",
    "amount": "10000000000000000000000",
    "description": "Fund developer grant"
  }'
```

### Vote on proposal

```bash
curl -X POST http://localhost:8333/vote \
  -H "Content-Type: application/json" \
  -d '{
    "proposal_id": 1,
    "voter_stake": 67000000000000000000000,
    "in_favor": true
  }'
```

## Docker Deployment

```dockerfile
FROM rust:1.74 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p koinon-node

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/koinon-node /usr/local/bin/
EXPOSE 8333 8334
ENTRYPOINT ["koinon-node"]
```

```bash
docker build -t tpt-oikos-node .
docker run -d \
  --name oikos-node \
  -p 8333:8333 \
  -p 8334:8334 \
  -v ~/.tpt-oikos:/data \
  tpt-oikos-node
```

## Docker Compose (Multi-Node)

```yaml
version: '3.8'
services:
  node1:
    build: .
    ports:
      - "8333:8333"
      - "8334:8334"
    volumes:
      - ./node1-data:/data
    command: ["--config", "/data/config.toml"]

  node2:
    build: .
    ports:
      - "8335:8333"
      - "8336:8334"
    volumes:
      - ./node2-data:/data
    command: ["--config", "/data/config.toml", "--seed", "node1:8334"]
    depends_on:
      - node1

  node3:
    build: .
    ports:
      - "8337:8333"
      - "8338:8334"
    volumes:
      - ./node3-data:/data
    command: ["--config", "/data/config.toml", "--seed", "node1:8334"]
    depends_on:
      - node1
```

## Monitoring

### Prometheus metrics

The node exposes Prometheus metrics at `http://localhost:8333/metrics`:

- `tpt_blocks_total` — Total blocks produced
- `tpt_transactions_total` — Total transactions processed
- `tpt_validators_active` — Active validator count
- `tpt_staked_total` — Total OIKOS staked
- `tpt_mempool_size` — Pending transaction count
- `tpt_peer_count` — Connected peer count

### Logging

```bash
# JSON logging for log aggregation
RUST_LOG=json ./target/release/koinon-node

# Filter specific modules
RUST_LOG=koinon_node=debug,koinon_dag=info ./target/release/koinon-node
```

## Troubleshooting

### "Database is locked"

SQLite allows one writer at a time. If you see this error:
- Ensure only one node process is using the data directory
- Check for stale lock files in the data directory

### "Port already in use"

```bash
# Find process using the port
lsof -i :8333

# Kill it
kill <PID>
```

### "Peer connection refused"

- Ensure the seed node is running
- Check firewall rules allow the P2P port (default 8334)
- Verify the seed address is correct

### Node won't start

```bash
# Check logs for errors
RUST_LOG=debug ./target/release/koinon-node 2>&1 | head -50

# Verify config file
cat ~/.tpt-oikos/config.toml
```

## Security Considerations

1. **RPC API** — By default, the RPC API has no authentication. For production, add API key authentication or firewall rules.
2. **P2P port** — Only expose the P2P port (8334) to trusted peers.
3. **Data directory** — Use appropriate file permissions (700) for the data directory.
4. **TLS** — For public deployments, place a TLS-terminating reverse proxy (nginx, Caddy) in front of the RPC API.
