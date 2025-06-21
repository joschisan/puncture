# Puncture

A daemon with integrated LDK node serving as a backend for the Puncture Flutter app that you can find at https://github.com/joschisan/puncture-app. The app communicates with this daemon via direct QUIC connections that are established via hole-punching. The connections are encrypted and autheticated via static ED25519 keys that identify both the daemon instance and the user to each other. Therefore the daemon does not require a public ip to accept incoming connections and can be deployed on any machine with an internet connection - without configuring TLS or networking in any way. Any machine with internet access and docker installed can deploy a daemon instance within minutes using our referce docker compose file linked below.

## Features

- **Single Binary**: The daemon contains an embedded LDK lightning node
- **No Domain Registration**: Daemon does not require a domain, public ip or TLS
- **Hole Punching**: Direct peer-to-peer connections without port forwarding
- **Ed25519 End-to-End Encryption**: Secure client to server communication using Ed25519
- **Built on Iroh**: Uses [Iroh](https://iroh.computer/) for networking and QUIC transport
- **Admin CLI Tool**: Comprehensive command-line interface for all administrative operations

⚠️ **Beta Status**: Not recommended for use with significant amounts

## Deploy with Docker

Download our reference docker-compose.yml with

```bash
curl -O https://raw.githubusercontent.com/joschisan/puncture/main/docker-compose.yml
```

and substitute your daemon instance name to be displayed to your users. Then you can deploy the daemon with 

```bash
docker-compose up -d
```

## Quickstart

### Accesing the Admin CLI

The `puncture-cli` binary is included in the Docker container and available in the PATH. Open an interactive shell inside the container via:

```bash
docker exec -it puncture-daemon bash
```

Or run cli commands directly like:

```bash
docker exec puncture-daemon puncture-cli --help
```

### Invite Users

Users need an invite code to connect to your daemon. Each invite code has an expiration time and a limit for the number of users that may register with it. You can create an invite code with defaults via:

```bash
puncture-cli user invite
```

or set custom a custom expiration and user limit with

```bash
puncture-cli user invite --expiry-days 1 --user-limit 10
```

### Inbound Liquidity Setup

Now your daemon needs inbound liquidity such that your uses can start receiving payments. You can purchase an incoming channel from Lightning Service Providers (LSPs). We recommend [LN Big](https://lnbig.com) as a reliable option.

Most LSPs require your lightning node to have onchain balance, so you'll need to fund your node first. You can generate an onchain address for your LDK Node with.

```bash
puncture-cli ldk onchain receive
```

Send Bitcoin to the generated address. Then get your node ID to request a incoming channel from the LSP:

```bash
puncture-cli ldk node-id
```

After requesting a channel with your node ID you might be asked to connect to the LSP's node in order complete the process:

```bash
puncture-cli ldk peer connect --node-id [LSP_NODE_ID] --address [LSP_ADDRESS] --persist
```

Replace `[LSP_NODE_ID]` and `[LSP_ADDRESS]` with the details provided by your chosen LSP. Once the process is complete you can monitor the confirmation of your channel via 

```bash
puncture-cli ldk channel list
```

**Only once the channel is confirmed your users will be able to generate Bolt12 Offers.**

If you only have a single incoming channel draining your user's entire overall balance might conflict with channel reserves, meaning outgoing payments might fail if the sum of all user balances approaches towards zero. This can be mitigated by maintaining a buffer of sats in your personal user account or by opening an outgoing channel for a total of two channels.

A common approach would be to open a channel to you already connected LSP Node via:

```bash
puncture-cli ldk channel open --node-id [LSP_NODE_ID] --address [LSP_ADDRESS] --channel-amount-sats 1000000
```

## Interfaces

The daemon listens on network interfaces:

- **0.0.0.0:8080**: Client Interface for user operations (configurable via `CLIENT_BIND`)
- **0.0.0.0:8081**: Lightning P2P network (configurable via `LDK_BIND`)  
- **0.0.0.0:8082**: Admin CLI HTTP service (configurable via `CLI_BIND`, **never expose publicly**)
- **0.0.0.0:8083**: Admin UI dashboard (configurable via `UI_BIND`, **never expose publicly**)

⚠️ **Security Warning**: The admin CLI and UI interfaces default to `0.0.0.0` for container compatibility, but should **NEVER** be exposed to the public internet. Always use `127.0.0.1` bindings in production as we do in our reference docker-compose.yml.

## Daemon Configuration Reference

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
| `LOG_LEVEL` | info | The log level, can be set to either error, warn, info, debug or trace. 
| `FEE_PPM` | 5000 | Fee rate in parts per million (PPM) applied to outgoing Lightning payments |
| `BASE_FEE_MSAT` | 10000 | Fixed base fee in millisatoshis added to all outgoing Lightning payments |
| `INVOICE_EXPIRY_SECS` | 3600 | Expiration time in seconds for all generated Lightning invoices |
| `CLIENT_BIND` | 0.0.0.0:8080 | Network address and port for the client interface to bind to |
| `LDK_BIND` | 0.0.0.0:8081 | Network address and port for the Lightning node to listen for peer connections |
| `CLI_BIND` | 0.0.0.0:8082 | Network address and port for the CLI interface (**never expose publicly**) |
| `UI_BIND` | 0.0.0.0:8083 | Network address and port for the UI interface (**never expose publicly**) |
| `MIN_AMOUNT_SATS` | 1 | Minimum amount in satoshis enforced across all incoming and outgoing payments |
| `MAX_AMOUNT_SATS` | 100000 | Maximum amount in satoshis enforced across all incoming and outgoing payments |
| `MAX_PENDING_PAYMENTS_PER_USER` | 10 | Maximum number of pending invoices and outgoing payments each user can have simultaneously |

⚠️ **Security Note**: Admin interfaces (`CLI_BIND` and `UI_BIND`) default to `0.0.0.0` for Docker compatibility but must never be exposed to the public internet. Always use `127.0.0.1` bindings for production deployments as we do in our reference docker-compose.yml.

## Admin CLI Command Reference

Generate invite code (expires in 1 day, for a maximum of 10 users):

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

```bash
puncture-cli user invite --expiry-days 1 --user-limit 10
```

List registered users:

```bash
puncture-cli user list
```

