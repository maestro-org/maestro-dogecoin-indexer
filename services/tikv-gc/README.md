# tikv-gc

The **MVCC garbage-collection safepoint manager**.

TiKV retains old MVCC versions of every key until a *GC safepoint* advances
past them — and with no safepoint manager it never collects at all, so disk
grows without bound. tikv-gc owns the safepoint, advancing it only past
timestamps that no reader can still need: every indexer instance's most recent
committed block stays readable, and nothing younger than a safe-zone window is
ever collected.

tikv-gc is that manager: a one-shot binary, run on a schedule (a looping
service in the compose stack, a CronJob in production), that computes the
newest timestamp that is still safe to garbage collect up to, and submits it
to PD. "Safe" is the minimum over these invariants, derived from the indexer
cursor entries in Redis:

1. **Earliest chain tip** — never GC past the most recent committed block of
   any instance, so every instance's chain tip remains queryable.
2. **Safe zone** — never GC anything newer than `SAFE_ZONE_MILLIS` (default 10
   minutes), regardless of what the entries say.

(The Bitcoin stack adds a cross-instance common-block invariant for its
unified snapshot reads; this API resolves each instance group independently,
so that invariant does not apply here.)

## Running

```
tikv-gc --tikv-address <pd:2379> --redis-address <redis://host:6379>
```

| Flag | Env | Default |
|---|---|---|
| `--tikv-address` | `TIKV_ADDRESS` | `localhost:2379` |
| `--redis-address` | `REDIS_ADDRESS` | `redis://localhost:6379` |
| `--cleanup-locks` | `CLEANUP_LOCKS` | `false` |
| `--safe-zone-millis` | `SAFE_ZONE_MILLIS` | `600000` |

In the compose stack it runs automatically every 10 minutes; `make gc` triggers a
manual one-shot run.

Note: retained MVCC history is exactly "how long since the safepoint last
advanced" — run tikv-gc rarely to keep more history readable (at the cost of
disk), or frequently to keep TiKV compact.
