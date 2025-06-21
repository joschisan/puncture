# Puncture

A daemon with integrated LDK node serving as a backend for the Puncture Flutter app that you can find at https://github.com/joschisan/puncture-app. The user clients communicate with this daemon via direct QUIC connections that are established via hole-punching and are encrypted and autheticated via static ED25519 keys that identify both the daemon instance and the user. Therefore the daemon does not require a public ip to accept incoming connections and can be deployed on local hardware and without configuring TLS or networking in any way. Any machine with internet access and docker installed can deploy a daemon instance within minutes using our referce docker compose file linked below.

## Features

- **Single Binary**: Easy to deploy with Docker - no complex setup required
- **LDK Integration**: Built-in Lightning Development Kit node
- **Iroh Integration** : Daemon does not require public ip or TLS
- **CLI Tools**: Comprehensive command-line interface for all administrative operations

⚠️ **Beta Status**: Not recommended for use with significant amounts

## Deploy with Docker

Download our reference docker-compose.yml with

```bash
curl -O https://raw.githubusercontent.com/joschisan/puncture/main/docker-compose.yml
```

and substitute your admin secret and daemon instance name as displayed to your users.

## Daemon Configuration

### Required Environment Variables

| Env | Description |
|-----|-------------|
| `ADMIN_AUTH` | Bearer token for admin API access, used to authenticate administrative operations |
| `JWT_SECRET` | Secret key for signing and verifying user JWT tokens |
| `PUNCTURE_DATA_DIR` | Directory path for storing user account data in a SQLite database |
| `LDK_DATA_DIR` | Directory path for storing LDK node data in a SQLite database |
| `BITCOIN_NETWORK` | Bitcoin network to operate on, determines address formats and chain validation rules |
| `BITCOIN_RPC_URL` | Bitcoin Core RPC URL for chain data access |
| `ESPLORA_RPC_URL` | Esplora API URL for chain data access |
| `INSTANCE_NAME` | Daemon instance name as displayed to your users |

*Note: Either `BITCOIN_RPC_URL` or `ESPLORA_RPC_URL` must be provided, but not both.*

### Optional Environment Variables

| Env | Default | Description |
|-----|---------|-------------|
| `FEE_PPM` | 10000 | Fee rate in parts per million (PPM) applied to outgoing Lightning payments |
| `BASE_FEE_MSAT` | 50000 | Fixed base fee in millisatoshis added to all outgoing Lightning payments |
| `INVOICE_EXPIRY_SECS` | 3600 | Expiration time in seconds for all generated Lightning invoices |
| `API_BIND` | 0.0.0.0:8080 | Network address and port for the HTTP API server to bind to |
| `LDK_BIND` | 0.0.0.0:9735 | Network address and port for the Lightning node to listen for peer connections |
| `MIN_AMOUNT_SATS` | 1 | Minimum amount in satoshis enforced across all incoming and outgoing payments |
| `MAX_AMOUNT_SATS` | 100000 | Maximum amount in satoshis enforced across all incoming and outgoing payments |
| `MAX_PENDING_PAYMENTS_PER_USER` | 10 | Maximum number of pending invoices and outgoing payments each user can have simultaneously |
| `MAX_DAILY_NEW_USERS` | 20 | Maximum number of new user registrations allowed per 24-hour period |

## Install Puncture CLI

The Puncture CLI allows you to manage your puncture daemon. You can install the cli with:

```bash
cargo install --git https://github.com/joschisan/puncture puncture-cli
```

## CLI Commands

Get your node ID (share this with LSPs for inbound channels):

```bash
puncture-cli --api-url <URL> --auth <TOKEN> \
  ldk \
  node-id
```

Inspect your balances:

```bash
puncture-cli --api-url <URL> --auth <TOKEN> \
  ldk \
  balances
```

Generate receiving address:

```bash
puncture-cli --api-url <URL> --auth <TOKEN> \
  ldk \
  onchain \
  receive
```

Send on-chain payment:

```bash
puncture-cli --api-url <URL> --auth <TOKEN> \
  ldk \
  onchain \
  send --address bc1q... --amount-sats 100000 --fee-rate 10
```

Open channel to peer:

```bash
puncture-cli --api-url <URL> --auth <TOKEN> \
  ldk \
  channel \
  open --node-id 03abc... --address 127.0.0.1:9735 --channel-amount-sats 1000000
```

Close a channel:

```bash
puncture-cli --api-url <URL> --auth <TOKEN> \
  ldk \
  channel \
  close --channel-id <CHANNEL_ID>
```

List channels:

```bash
puncture-cli --api-url <URL> --auth <TOKEN> \
  ldk \
  channel \
  list
```

Connect to peer:

```bash
puncture-cli --api-url <URL> --auth <TOKEN> \
  ldk \
  peer \
  connect --node-id 03abc... --address 127.0.0.1:9735
```

Disconnect from peer:

```bash
puncture-cli --api-url <URL> --auth <TOKEN> \
  ldk \
  peer \
  disconnect --node-id <NODE_ID>
```

List connected peers:

```bash
puncture-cli --api-url <URL> --auth <TOKEN> \
  ldk \
  peer \
  list
```

