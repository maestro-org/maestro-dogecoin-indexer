# compressor-xdg

A Dogecoin **light node and enrichment engine**: the first stage of the indexing
pipeline. It connects directly to a Dogecoin Core node — over the P2P protocol
for block download and the JSON-RPC interface for chain metadata — stores blocks
in a local RocksDB database (the *rolldb*), and precomputes everything
downstream indexers need but can't get from raw blocks alone:

- **Resolved inputs**: every transaction input is resolved to the full previous
  output it spends, so consumers never need their own UTXO index.
- **Dunes**: etchings, mints, and per-output dune balances (Dogecoin's
  runes-protocol analogue, implemented in-tree under `src/storage/runes/dunes`).
- **Inscriptions & DRC-20**: inscription envelope parsing and DRC-20 state
  (via the BRC-20 state machine).

Satoshi-range (ordinals) tracking is disabled for Dogecoin: early block rewards
were randomly generated, which makes ordinal-style numbering ill-defined.

The result — blocks bundled with their *enrichment context* — is served over
gRPC (see [`proto/sync/v1/sync.proto`](../../proto/sync/v1/sync.proto)) to any
number of downstream indexer instances:

| RPC | Purpose |
|---|---|
| `PageBlocksWithContext` | Bulk paged download of historical blocks (initial sync) |
| `StreamUpdatesWithContext` | Apply/undo/reset stream at the chain tip (rollback-aware) |

Because inputs are resolved and metaprotocol state is precomputed **once**,
adding another indexer instance costs no extra node or UTXO-resolution work —
this is what makes running many parallel indexer instances cheap.

## Running

```
compressor-xdg <config.toml> <subcommand>
```

| Subcommand | Meaning |
|---|---|
| `sync` | Sync the rolldb from the node only |
| `serve` | Serve gRPC from an existing rolldb only |
| `daemon` | `sync` + `serve` (recommended) |
| `rollback <height>` | Force-roll the database back to a height (recovery tool) |

## Configuration

See [`configs/compressor/`](../../configs/compressor) for complete examples.

| Key | Meaning |
|---|---|
| `chain_db.path` | RocksDB rolldb directory |
| `chain_db.immutable_after_confs` | Blocks deeper than this are stored immutably (rollback log kept above it) |
| `sync.node_address` | Dogecoin Core P2P address (`host:22556` mainnet, `host:44556` testnet) |
| `sync.node_rpc` / `node_rpc_user` / `node_rpc_pass` | Dogecoin Core JSON-RPC endpoint and credentials |
| `sync.network` | `dogecoin` or `dogecoin_testnet` |
| `sync.health_endpoint` | HTTP health listener (`/health` reports sync progress) |
| `sync.first_rune_height` | Dunes activation height (5084000 on mainnet) |
| `sync.first_inscription_height` | Inscriptions activation height (4600000 on mainnet, 4250000 on testnet) |
| `sync.utxos_in_memory` | Keep the TXO resolver set in memory (faster; mainnet needs ~raw UTXO set worth of RAM) |
| `sync.block_page_size` / `sync.sync_channel_queue` | Paging and channel-depth tuning for the sync pipeline |
| `logging.max_level` | Log level (`info`, `debug`, ...) |
| `serve.listen_address` | gRPC listener address |

## Ports

- `50051` — gRPC sync service
- `50052` — HTTP health (`curl localhost:50052/health`)

## Note on the build

compressor-xdg intentionally builds **outside** the repo's cargo workspace (own
`Cargo.lock`): its Dogecoin block decoding uses a maintained `rust-bitcoin`
fork pinned to a version incompatible with the TiKV-side services. It only
shares the gRPC proto with them, so the split is harmless — `make build`
covers both.
