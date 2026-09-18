# mapi-xdg

The **HTTP API layer**: a stateless axum server that reads pre-indexed data out
of TiKV and serves it as a documented REST API. All indexed data is written by
[polyphony-xdg](../polyphony-xdg) instances; mapi-xdg's job is to (1) discover
which instances to read from, and (2) decode timbre keys/values into API
responses. A small group of endpoints (raw blocks, node info, mempool queries)
proxy the Dogecoin node's JSON-RPC directly.

The OpenAPI document (Swagger UI at `/swagger-ui`) covers the index-backed
surface: addresses, dunes, inscriptions and DRC-20. The node-proxy endpoints
(`/blocks/*`, `/general/info`, `/rpc/*`, `/transactions/*`) are served by the
same binary but are not part of the generated spec.

## How reads work

For each index-backed request, mapi-xdg looks up the per-instance-group
registry in Redis (`dogecoin:<network>:<instance>:scores` sorted sets) to find
a registered (dataplane, instance) pair for the reducer the endpoint needs,
then opens a TiKV snapshot at the current timestamp and scans that instance's
keyspace. Only instances that have fully caught up with the chain tip are
registered, so requests never read from a backfilling instance.

## Running

```
mapi-xdg --mode dogecoin-testnet --node-address http://127.0.0.1:44555 \
    --tikv-address 127.0.0.1:2379 --redis redis://127.0.0.1:6379
```

| Flag | Env | Default | Meaning |
|---|---|---|---|
| `--mode` | `MODE` | required | `dogecoin`, `dogecoin-testnet`, or `generate-open-api` (write the OpenAPI JSON and exit) |
| `--listen-address` | `LISTEN_ADDRESS` | `0.0.0.0:3000` | HTTP listener |
| `--node-address` | `NODE_ADDRESS` | required | Dogecoin Core JSON-RPC endpoint |
| `--node-user` / `--node-password` | `NODE_USER` / `NODE_PASSWORD` | `maestro` | Node RPC credentials |
| `--tikv-address` | `TIKV_PD_CLIENT` | `127.0.0.1:2379` | TiKV PD endpoint |
| `--redis` | `REDIS` | required | Redis connection URL |

## Security note

mapi-xdg performs **no authentication or rate limiting in-process** — deploy it
behind a gateway if you expose it publicly.
