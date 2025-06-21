# Puncture

A daemon with integrated LDK node serving as a backend for the Puncture Flutter app that you can find at https://github.com/joschisan/puncture-app. The user clients communicate with this daemon via direct QUIC connections that are established via hole-punching and are encrypted and autheticated via static ED25519 keys that identify both the daemon instance and the user. Therefore the daemon does not require a public ip to accept incoming connections and can be deployed on local hardware and without configuring TLS or networking in any way. Any machine with internet access and docker installed can deploy a daemon instance within minutes using our referce docker compose file linked below.

## Features

- **Iroh Integration**: Daemon does not require public ip or TLS
- **Single Binary**: Easy to deploy with Docker
- **LDK Integration**: Built-in LDK node
- **Admin CLI Tool**: Comprehensive command-line interface for all administrative operations

⚠️ **Beta Status**: Not recommended for use with significant amounts

## Deploy with Docker

Download our reference docker-compose.yml with

```bash
curl -O https://raw.githubusercontent.com/joschisan/puncture/main/docker-compose.yml
```

and substitute your daemon instance name as displayed to your users.

## Interfaces

The daemon listens on network interfaces:

- **0.0.0.0:8080**: Public Iroh API for user operations (exposed)
- **0.0.0.0:9735**: Lightning P2P network (exposed)  
- **127.0.0.19090**: Admin cli http service (localhost only, **never publicly exposed**)

The admin cli network interface is **hardcoded to `127.0.0.1:9090`** in both the daemon and CLI for security. This ensures the admin interface can never be accidentally exposed to the internet, even with misconfigurations.

### Using the Admin CLI

The `puncture-cli` binary is included in the Docker container and available in the PATH. Access it via:

```bash
# Get interactive shell in container
docker exec -it puncture-daemon bash

# Use CLI commands (no auth needed - secured by network isolation)
puncture-cli invite --expiry-days 7
```

Or run commands directly:

```bash
# Directly from host
docker exec puncture-daemon puncture-cli invite --expiry-days 7
```

## Daemon Configuration

### Required Environment Variables

| Env | Description |
|-----|-------------|
| `PUNCTURE_DATA_DIR` | Directory path for storing user account data in a SQLite database |
| `LDK_DATA_DIR` | Directory path for storing LDK node data in a SQLite database |
| `BITCOIN_NETWORK` | Bitcoin network to operate on, determines address formats and chain validation rules |
| `BITCOIN_RPC_URL` | Bitcoin Core RPC URL for chain data access |
| `ESPLORA_RPC_URL` | Esplora API URL for chain data access |
| `DAEMON_NAME` | Daemon instance name as displayed to your users |

*Note: Either `BITCOIN_RPC_URL` or `ESPLORA_RPC_URL` must be provided, but not both.*

### Optional Environment Variables

| Env | Default | Description |
|-----|---------|-------------|
| `FEE_PPM` | 10000 | Fee rate in parts per million (PPM) applied to outgoing Lightning payments |
| `BASE_FEE_MSAT` | 50000 | Fixed base fee in millisatoshis added to all outgoing Lightning payments |
| `INVOICE_EXPIRY_SECS` | 3600 | Expiration time in seconds for all generated Lightning invoices |
| `API_BIND` | 0.0.0.0:8080 | Network address and port for the Iroh API endpoint to bind to |
| `LDK_BIND` | 0.0.0.0:9735 | Network address and port for the Lightning node to listen for peer connections |
| `MIN_AMOUNT_SATS` | 1 | Minimum amount in satoshis enforced across all incoming and outgoing payments |
| `MAX_AMOUNT_SATS` | 100000 | Maximum amount in satoshis enforced across all incoming and outgoing payments |
| `MAX_PENDING_PAYMENTS_PER_USER` | 10 | Maximum number of pending invoices and outgoing payments each user can have simultaneously |

*Note: The admin interface always binds to `127.0.0.1:9090` and is not configurable for security reasons.*

## CLI Commands

**All commands must be run inside the container or via docker exec. The CLI connects to the hardcoded admin interface at `127.0.0.1:9090`.**

Generate invite code (expires in 7 days, max 10 users):

```bash
puncture-cli invite --expiry-days 7 --user-limit 100
```

Get your node ID (share this with LSPs for inbound channels):

```bash
puncture-cli ldk node-id
```

Inspect your balances:

```bash
puncture-cli ldk balances
```

Generate receiving address:

```bash
puncture-cli ldk onchain receive
```

Send on-chain payment:

```bash
puncture-cli ldk onchain send --address bc1q... --amount-sats 100000 --fee-rate 10
```

Open channel to peer:

```bash
puncture-cli ldk channel open --node-id 03abc... --address 127.0.0.1:9735 --channel-amount-sats 1000000
```

Close a channel:

```bash
puncture-cli ldk channel close --user-channel-id 12345 --counterparty-node-id 03abc... 
```

List channels:

```bash
puncture-cli ldk channel list
```

Connect to peer:

```bash
puncture-cli ldk peer connect --node-id 03abc... --address 127.0.0.1:9735
```

Disconnect from peer:

```bash
puncture-cli ldk peer disconnect --counterparty-node-id 03abc...
```

List connected peers:

```bash
puncture-cli ldk peer list
```

List registered users:

```bash
puncture-cli user list
```

