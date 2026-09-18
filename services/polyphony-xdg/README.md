# polyphony-xdg

The **indexer**: consumes enriched blocks from [compressor-xdg](../compressor-xdg)
over gRPC, runs them through a configurable set of **reducers**
(map/reduce-style state machines, one per query pattern), and writes the
resulting key/value mutations to TiKV — plus cursor and instance-registry
entries to Redis so the API layer knows what data exists and how fresh it is.

Began as a fork of TxPipe's Scrolls and has since been substantially rewritten;
built on the [gasket](https://github.com/construkts/gasket-rs) staged-pipeline
framework:

```
source (compressor gRPC / emulator) ──► reducers (fan-out per block) ──► storage (TiKV + Redis)
```

## Key properties

- **Multi-instance by design.** Each instance is identified by
  (`dataplane_id`, `instance_id`) and namespaces every TiKV key it writes with
  that identity (the key layout lives in [timbre-xbt](../../crates/timbre-xbt),
  shared with the Bitcoin stack). Any number of instances — different reducer
  sets, different code versions — can index the same chain into the same TiKV
  cluster concurrently, all fed by one compressor.
- **Registry-gated swap-over.** An instance advertises itself in the
  per-instance-group Redis registry (`dogecoin:<network>:<instance>:scores`)
  **only once it reaches the mutable window at the chain tip**. Until then the
  API layer doesn't know it exists. This is the zero-downtime upgrade
  mechanism: deploy a new instance with patched reducers, let it backfill in
  parallel, and it takes over automatically when caught up. Set
  `storage.advertise_immediately = true` to bypass the gate in dev setups.
- **Rollback-aware.** Inverse actions for recent blocks are buffered (and
  persisted to TiKV), so chain reorgs unwind cleanly; a reorg deeper than the
  buffer is unrecoverable and requires an instance resync.

## Running

```
polyphony-xdg daemon --config <config.toml> [--console plain|tui]
```

## Configuration

Layered: config file via `--config`, then env vars prefixed `POLYPHONY` with
`__` separator (e.g. `POLYPHONY__STORAGE__INSTANCE_ID=1`). Complete examples in
[`configs/polyphony/`](../../configs/polyphony).

| Section | Keys |
|---|---|
| `[general]` | `stage_timeout_secs`, `stage_message_queue`, `buffer_size` (rollback buffer depth) |
| `[source]` | `type = "Compressor"` with `url`, `max_items_per_page` |
| `[intersect]` | Where to start: `Origin`, `Tip`, or `Point = [height, "hash"]` |
| `[storage]` | `type = "TiKV"`: `connection_params` (PD endpoint), `redis_address`, `network` (`mainnet`/`testnet`), `dataplane_id`, `instance_id`, `advertise_immediately`, plus commit tuning (`sync_batch_size`, `tikv_commit_*`, `tikv_cleanup_locks`) |
| `[[reducers]]` | One `type = "..."` entry per reducer to run — see [docs/reducers.md](../../docs/reducers.md) |

polyphony-xdg exposes no network listener of its own; observe it through its
logs, its Redis cursor entries, or the TUI console.
