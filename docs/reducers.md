# Reducers

A reducer is one query pattern, indexed. Each one consumes enriched blocks and
emits key/value actions; enabling one is a single `[[reducers]]` entry in the
polyphony config. Because compressor resolves inputs and precomputes
metaprotocol state, most reducers are short — they read facts off the enriched
block rather than computing them.

Each reducer belongs to an *instance group* — the name a polyphony instance
running it advertises in the Redis registry, and the name the API layer
resolves when serving requests that need it.

## Catalogue

### Address / UTXO

| Reducer | Instance group | Answers |
|---|---|---|
| `UtxosByScriptHash` | `utxos` | UTXOs held by an address/script |
| `TxsByScriptHash` | `transactions` | Transaction history of an address/script |
| `SatBalanceByScriptHash` | `sat-balance-by-script-hash` | Confirmed balance per address/script |
| `ScriptByScriptHash`, `ScriptHashByAddressPayloadHash` | `scripts` | Script and address-payload resolution |

### Dunes

| Reducer | Instance group | Answers |
|---|---|---|
| `EtchingByRuneId` | `dunes` | A dune's etching (name, symbol, terms) |
| `MintsByRuneId` | `dunes` | Mint history of a dune |
| `UtxosByRuneId` | `dunes` | UTXOs carrying a dune |
| `RunesByUtxo` | `dunes` | Dune balances of one UTXO |
| `RuneIdByRuneName` | `dunes` | Name → dune id |
| `RuneBalancesByScriptHash` | `dunes` | Per-address dune balances |
| `BalancesByRuneId` | `balances-by-rune-id` | Holder list of a dune |

### Inscriptions / DRC-20

| Reducer | Instance group | Answers |
|---|---|---|
| `InscriptionsByScriptHash` | `inscriptions` | Inscriptions held by an address |
| `InscriptionsByUtxo` | `inscriptions` | Inscriptions located on a UTXO |
| `TransferInscriptionsByScriptHash` | `inscriptions` | DRC-20 transfer inscriptions per address |
| `InscriptionUtxosByScriptHash` | `inscription-utxos-by-script-hash` | UTXOs carrying inscriptions per address |
| `ContentByInscriptionId` | `content-by-inscription-id` | Inscription content bytes |
| `BalancesByBrc20` | `inscriptions` | Holder balances per DRC-20 ticker |
| `Brc20BalancesByScriptHash` | `inscriptions` | DRC-20 balances of one address |
| `Brc20TermsByTicker` | `inscriptions` | DRC-20 deploy terms |

## Writing one

A reducer implements a fold from an enriched block (and its resolved inputs)
to `ReducerOutput` values, which the storage stage translates into key/value
actions. The steps:

1. Add a module under `services/polyphony-xdg/src/reducers/` implementing the
   fold, with a `Config` struct for any options.
2. Register it in the `Reducer`/`Config`/`ReducerOutput` enums in
   `reducers/mod.rs`, and map it to an instance group in
   `Config::instance_name`.
3. Define its key/value encoding in `crates/timbre-xbt` (a new reducer tag
   byte and the codec for its key body) — field order is the query plan, since
   range scans over the encoded keys are the only access path.
4. Add the read path in `services/mapi-xdg` (an encoder method on
   `PolyphonyWrapper` and a route that scans and decodes).

Deploy the change as a *new instance* (bump `instance_id`), let it backfill,
and it takes over from the old instance the moment it reaches the tip.
