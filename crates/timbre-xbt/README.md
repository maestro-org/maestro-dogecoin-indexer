# timbre-xbt

The TiKV key/value encoding schema shared by the indexer (polyphony) and the
API layer (mapi): every key layout, value codec and reducer tag byte in the
store. The two services never talk to each other — this crate is the contract
between them. The schema is shared with the Bitcoin stack, which is why rune
terminology appears alongside Dogecoin's dunes.

**Note:** Max Key Size does not include 5 bytes used for dataplane ID, instance ID, reducer tag, initial `BREAK`.

| Reducer  | Tag Byte | Max Key Size | Max Val Size |
| --- | --- | --- | --- |
| EtchingByRuneId | `0x61` | 12 | 163 |
| MintsByRuneId  | `0x62` | 12 | 16 |
| UtxosByRuneId  | `0x63` | 59 | 46 |
| UtxosByScriptHash  | `0x64` | 67 | 8 |
| RunesByUtxo  | `0x65` | 37 | 8 + (number of runes * 28) |
| ~~RuneInfoByRuneId~~ | `0x66` | - | - |
| ScriptByScriptHash | `0x67` | 20 | 4 + script length |
| ScriptHashByAddressPayloadHash | `0x68` | 20 | 20 |
| RuneIdByRuneName | `0x69` | 16 | 12 |
| InscriptionsByUtxo | `0x6A` | 37 | 4 + (number of inscriptions * 40) |
| Brc20TotalBalanceByScriptHash | `0x6B` | 27 | 16 |
| Brc20AvailableBalanceByScriptHash | `0x6C` | 27 | 16 |
| Brc20TermsByTicker | `0x6D` | 6 | 74 |
| BalancesByBrc20 | `0x6E` | 27 | 16 |
| TxsByScriptHash | `0x6F` | 65 | 3 |
| RuneBalancesByScriptHash | `0x70` | 33 | 16 |
| InscriptionsByScriptHash | `0x71` | 57 | 51 |
| ContentByInscriptionId | `0x72` | 36 | 27 + content type length + content body length |
| TransferInscriptionsByScriptHash | `0x73` | 64 | 77 |
| SatsPerVbByBlock | `0x74` | 8 | 26 |
| InscriptionUtxosByScriptHash | `0x75` | 89 | 35 + (number of inscriptions * 66) |
| ~~InscriptionActivityByBlock~~ | `0x76` | - | - |
| BlockByTxHash | `0x77` | 32 | 35 |
| HeightByBlockHash | `0x78` | 32 | 17 |
| InscriptionActivityByTx | `0x79` | 35 | 50 + (num of inscriptions * 158)
| TxsByInscription | `0x7A` | 20 | 106 * (num of inscription activities entries)
| TxInfo | `0x7B` | 35 | 146 + (242 * num of inputs) + (191 * num of outputs)
| BlockInfo | `0x7C` | 17 | 164 + length of script_sig in coinbase tx
| TxsByBlock | `0x7D` | 35 | 32
| BalancesByRuneId | `0x7E` | 55 | 16
| SpendingTxByTxo | `0x7F` | 50 | 35
| (reserved) | `0x80` | — | retired reducer |
| RuneUtxosByScriptHash | `0x81` | 89 | 22 + (number of different rune kinds * 51)
| TxsByRuneId | `0x82` | 59 | 18 + num of self-transferring addresses * 36 + num of sender addresses * 36 + num of receiver addresses * 36
| TxFirstSeenTimestamp | `0x83` | 32 | 17
| RuneTxsByScriptHash | `0x84` | 67 | 54 + number of runes with increased balance * 28 + number of runes with decreased balance * 28
| SatBalanceByScriptHash | `0x85` | 20 | 8
| SatTxsByScriptHash | `0x86` | 67 | 10
| InscriptionActivityByScriptHash| `0x87` | 67 | 14 + number of self-transfers * 56 + number of sent inscriptions * 77 + number of received inscriptions * 80

**Current maximum key size:** 89 (108 including namespace etc)