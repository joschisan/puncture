#!/bin/bash
set -e  # Exit immediately if a command exits with a non-zero status

# Create data directory if it doesn't exist
mkdir -p "./data-dir-testing"

# Set up trap to handle cleanup on exit (whether success or failure)
cleanup() {
  echo "Cleaning up..."
  
  # Kill daemons started by the test
  pkill -f "puncture-daemon" || true
  
  # Remove data directories
  rm -rf "./data-dir-testing"
  
  # Stop and remove bitcoind container
  docker stop puncture-bitcoind || true
  docker rm puncture-bitcoind || true
}

# Run cleanup on script exit (normal or error)
trap cleanup EXIT

# Start bitcoind container
docker run -d \
  --name puncture-bitcoind \
  -p 18443:18443 \
  -p 18444:18444 \
  ruimarinho/bitcoin-core:latest \
  -regtest=1 \
  -server=1 \
  -rpcuser=bitcoin \
  -rpcpassword=bitcoin \
  -rpcallowip=0.0.0.0/0 \
  -rpcbind=0.0.0.0

# Build the entire workspace
cargo build

# Run the testing binary with logging
RUST_LOG=info cargo run -p puncture-testing 