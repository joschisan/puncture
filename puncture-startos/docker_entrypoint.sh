#!/bin/bash
set -e

export PUNCTURE_DATA_DIR=/data/puncture
export LDK_DATA_DIR=/data/ldk
export BITCOIN_NETWORK=bitcoin
export BITCOIN_RPC_URL="http://bitcoind.embassy:8332"
export DAEMON_NAME=$(yq .daemon-name /data/start9/config.yaml)

exec puncture-daemon "$@" 