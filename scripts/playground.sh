#!/usr/bin/env bash
# Playground deployment: run the Rust services natively (fast edit-compile-run loop)
# against a local TiKV from `tiup playground`, with dogecoind and Redis in docker.
#
# Prerequisites: docker, cargo, and tiup (https://tiup.io):
#   curl --proto '=https' --tlsv1.2 -sSf https://tiup-mirrors.pingcap.com/install.sh | sh
#
# Usage:
#   scripts/playground.sh up      # start infra (tiup tikv-slim, dogecoind, redis)
#   scripts/playground.sh run     # build + run compressor, polyphony, mapi (logs in ./tmp/playground)
#   scripts/playground.sh down    # stop everything
set -euo pipefail

cd "$(dirname "$0")/.."
LOGDIR=tmp/playground
mkdir -p "$LOGDIR"

up() {
    echo "--> starting TiKV playground (pd :2379)"
    nohup tiup playground --mode tikv-slim --without-monitor \
        >"$LOGDIR/tiup.log" 2>&1 &
    echo $! > "$LOGDIR/tiup.pid"

    echo "--> starting dogecoind (testnet, rpc :44555) and redis (:6379)"
    docker build -t xdg-playground-dogecoin configs/dogecoin
    docker run -d --name xdg-playground-dogecoin \
        -p 44555:44555 -p 44556:44556 \
        -v xdg-playground-dogecoin:/data \
        xdg-playground-dogecoin \
        -datadir=/data -testnet -server -rpcbind=0.0.0.0 -rpcallowip=0.0.0.0/0 \
        -rpcuser=maestro -rpcpassword=maestro -printtoconsole

    docker run -d --name xdg-playground-redis \
        -p 6379:6379 \
        redis:7.2 redis-server --appendonly no --save ""

    echo "--> waiting for pd to come up"
    until curl -sf http://127.0.0.1:2379/pd/api/v1/version >/dev/null 2>&1; do sleep 1; done
    echo "playground infra up. Next: scripts/playground.sh run"
}

run() {
    echo "--> building services (release)"
    (cd services/compressor-xdg && cargo build --release)
    cargo build --release -p polyphony-xdg -p mapi-xdg

    echo "--> starting compressor-xdg (logs: $LOGDIR/compressor.log)"
    nohup ./services/compressor-xdg/target/release/compressor-xdg \
        configs/compressor/playground.toml daemon \
        >"$LOGDIR/compressor.log" 2>&1 &
    echo $! > "$LOGDIR/compressor.pid"

    echo "--> starting polyphony-xdg (logs: $LOGDIR/polyphony.log)"
    nohup ./target/release/polyphony-xdg daemon --console plain \
        --config configs/polyphony/playground.toml \
        >"$LOGDIR/polyphony.log" 2>&1 &
    echo $! > "$LOGDIR/polyphony.pid"

    echo "--> starting mapi-xdg on :3000 (logs: $LOGDIR/mapi.log)"
    nohup ./target/release/mapi-xdg \
        --mode dogecoin-testnet \
        --node-address http://127.0.0.1:44555 \
        --tikv-address 127.0.0.1:2379 \
        --redis redis://127.0.0.1:6379 \
        >"$LOGDIR/mapi.log" 2>&1 &
    echo $! > "$LOGDIR/mapi.pid"

    echo "services running. Try: curl -s localhost:50052/health | jq"
}

down() {
    for svc in mapi polyphony compressor tiup; do
        if [ -f "$LOGDIR/$svc.pid" ]; then
            kill "$(cat "$LOGDIR/$svc.pid")" 2>/dev/null || true
            rm -f "$LOGDIR/$svc.pid"
        fi
    done
    pkill -f "tiup-playground" 2>/dev/null || true
    docker rm -f xdg-playground-dogecoin xdg-playground-redis 2>/dev/null || true
    echo "playground stopped."
}

case "${1:-}" in
    up) up ;;
    run) run ;;
    down) down ;;
    *) echo "usage: $0 {up|run|down}"; exit 1 ;;
esac
