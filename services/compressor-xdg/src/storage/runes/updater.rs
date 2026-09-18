use std::{collections::HashMap, u128};

use bitcoin::{hashes::Hash, BlockHash};
use ordinals::Height;
// use ordinals::{Artifact, Edict, Etching, Height, Rune, RuneId, Runestone};
use rocksdb::{OptimisticTransactionDB, Transaction};
use tracing::{error, warn};

use crate::{
    storage::{
        self, BlockHeight, ChainDB, DBBytes, DBInt, DBSerde, DBUInt128, Error, KVTable, TxoBody,
        TxoRef,
    },
    sync::BitcoinCompatibleNetwork,
};

use super::{
    dunes::{
        self, chain::Chain, claim, dune::Dune, dune_id::DuneId, dunestone::Dunestone, edict::Edict,
        etching::Etching, terms::Terms, Allocation,
    },
    DuneIdByNameKV, DuneMintsByIdKV, DuneTerms, DuneTermsByIdKV, DunesReservedCountersKV,
    DUNES_RESERVED_COUNTERS_KEY,
};

pub struct IndexRunesResult {
    // hashmap of runes to amounts for each output
    pub output_dunes: Vec<HashMap<u128, u128>>,
    // etching was present and successful, dune name
    pub successful_etch: Option<u128>,
    // mint at claim edict (sorted and dedup) index was successful
    pub successful_mints: Vec<u16>,
}

pub fn index_runes(
    resolver: &HashMap<TxoRef, TxoBody>,
    chain_db: &ChainDB,
    db_tx: &Transaction<OptimisticTransactionDB>,
    tx_index: u32,
    tx: &bitcoin::Transaction,
    height: BlockHeight,
    network: BitcoinCompatibleNetwork,
) -> Result<IndexRunesResult, Error> {
    let mut successful_etch = None;
    let mut successful_mints = vec![];

    let dunestone = Dunestone::from_transaction(tx);

    let cenotaph = dunestone
        .as_ref()
        .map(|dunestone| dunestone.cenotaph)
        .unwrap_or_default();

    let default_output = dunestone.as_ref().and_then(|dunestone| {
        dunestone
            .pointer
            .and_then(|default| usize::try_from(default).ok())
    });

    let mut unallocated = unallocated(tx, resolver)?;

    // A vector of allocated transaction output dune balances
    let mut allocated: Vec<HashMap<u128, u128>> = vec![HashMap::new(); tx.output.len()];

    if let Some(dunestone) = dunestone {
        let mut allocation = match dunestone.etching {
            Some(etching) => {
                let dune = etched(
                    &chain_db, &db_tx, &resolver, tx_index, tx, &dunestone, height, network,
                )?;

                if let Some(dune) = dune {
                    successful_etch = Some(dune.1 .0);
                    // Construct an allocation, representing the new dunes that may be
                    // allocated. Beware: Because it would require constructing a block
                    // with 2**16 + 1 transactions, there is no test that checks that
                    // an eching in a transaction with an out-of-bounds index is
                    // ignored.
                    match u16::try_from(tx_index) {
                        Ok(_) => Some(Allocation {
                            balance: u128::MAX,
                            divisibility: etching.divisibility.unwrap_or_default(),
                            id: u128::from(height) << 16 | u128::from(tx_index),
                            dune: dune.1,
                            premine: etching.premine.unwrap_or_default(),
                            spacers: etching.spacers.unwrap_or_default(),
                            symbol: etching.symbol,
                            mint: etching.terms.map(|mint| Terms {
                                cap: mint.cap,
                                limit: mint.limit.map(|limit| limit.clamp(0, dunes::MAX_LIMIT)),
                                height: mint.height,
                                offset: mint.offset,
                            }),
                            turbo: etching.turbo,
                        }),
                        Err(_) => None,
                    }
                } else {
                    None
                }
            }
            None => None,
        };

        let mut premine_amount = 0;

        if !cenotaph {
            let mut mintable: HashMap<u128, u128> = HashMap::new();

            let mut claims = dunestone
                .edicts
                .iter()
                .filter_map(|edict| claim(edict.id))
                .collect::<Vec<u128>>();
            claims.sort();
            claims.dedup();

            for (idx, id) in claims.into_iter().enumerate() {
                if let Ok(key) = DuneId::try_from(id) {
                    // check if mintable at this height and get mint limit
                    if let Some(x) = mint(&chain_db, &db_tx, key, height)? {
                        mintable.insert(id, x);
                        successful_mints.push(idx as u16)
                    } else {
                        continue;
                    }
                }
            }

            let limits = mintable.clone();

            for Edict { id, amount, output } in dunestone.clone().edicts {
                let Ok(output) = usize::try_from(output) else {
                    continue;
                };

                // Skip edicts not referring to valid outputs
                if output >= tx.output.len() {
                    continue;
                }

                let (balance, id) = if id == 0 {
                    // If this edict allocates new issuance dunes, skip it
                    // if no issuance was present, or if the issuance was invalid.
                    // Additionally, replace ID 0 with the newly assigned ID, and
                    // get the unallocated balance of the issuance.
                    match allocation.as_mut() {
                        Some(Allocation { balance, id, .. }) => {
                            premine_amount = premine_amount + amount;
                            (balance, *id)
                        }
                        None => continue,
                    }
                } else if let Some(claim) = claim(id) {
                    match mintable.get_mut(&claim) {
                        Some(balance) => (balance, claim),
                        None => continue,
                    }
                } else {
                    // Get the unallocated balance of the given ID
                    match unallocated.get_mut(&id) {
                        Some(balance) => (balance, id),
                        None => continue,
                    }
                };

                let mut allocate = |balance: &mut u128, amount: u128, output: usize| {
                    if amount > 0 {
                        *balance -= amount;
                        *allocated[output].entry(id).or_default() += amount;
                    }
                };

                if output == tx.output.len() {
                    // find non-OP_RETURN outputs
                    let destinations = tx
                        .output
                        .iter()
                        .enumerate()
                        .filter_map(|(output, tx_out)| {
                            (!tx_out.script_pubkey.is_op_return()).then_some(output)
                        })
                        .collect::<Vec<usize>>();

                    if amount == 0 {
                        // if amount is zero, divide balance between eligible outputs
                        let amount = *balance / destinations.len() as u128;
                        let remainder =
                            usize::try_from(*balance % destinations.len() as u128).unwrap();

                        for (i, output) in destinations.iter().enumerate() {
                            allocate(
                                balance,
                                if i < remainder { amount + 1 } else { amount },
                                *output,
                            );
                        }
                    } else {
                        // if amount is non-zero, distribute amount to eligible outputs
                        for output in destinations {
                            allocate(balance, amount.min(*balance), output);
                        }
                    }
                } else {
                    // Get the allocatable amount
                    let amount = if amount == 0 {
                        *balance
                    } else {
                        amount.min(*balance)
                    };

                    allocate(balance, amount, output);
                }
            }

            // increment entries with minted dunes
            for (id, amount) in mintable {
                let minted = limits[&id] - amount;
                if minted > 0 {
                    let id = DuneId::try_from(id).unwrap();

                    let current_mints =
                        DuneMintsByIdKV::get_by_key(&chain_db.db, db_tx, id.clone().into())?
                            .map(|DBUInt128(x)| x)
                            .unwrap_or_default();

                    let new_mints = current_mints + 1;

                    DuneMintsByIdKV::stage_upsert(
                        &chain_db.db,
                        id.into(),
                        DBUInt128(new_mints),
                        db_tx,
                    )?;
                }
            }
        }

        if let Some(Allocation { id, dune, .. }) = allocation {
            let id = DuneId::try_from(id).unwrap();

            create_rune_entry(&chain_db, &db_tx, &dunestone, id, dune, height)?;
        }
    }

    let mut burned: HashMap<u128, u128> = HashMap::new();

    if cenotaph {
        for (id, balance) in unallocated {
            *burned.entry(id).or_default() += balance;
        }
    } else {
        // assign all un-allocated dunes to the default output, or the first non
        // OP_RETURN output if there is no default, or if the default output is
        // too large
        if let Some(vout) = default_output
            .filter(|vout| *vout < allocated.len())
            .or_else(|| {
                tx.output
                    .iter()
                    .enumerate()
                    .find(|(_vout, tx_out)| !tx_out.script_pubkey.is_op_return())
                    .map(|(vout, _tx_out)| vout)
            })
        {
            for (id, balance) in unallocated {
                if balance > 0 {
                    *allocated[vout].entry(id).or_default() += balance;
                }
            }
        } else {
            for (id, balance) in unallocated {
                if balance > 0 {
                    *burned.entry(id).or_default() += balance;
                }
            }
        }
    }

    // // update outpoint balances
    // let mut buffer: Vec<u8> = Vec::new();
    // for (vout, balances) in allocated.into_iter().enumerate() {
    //   if balances.is_empty() {
    //     continue;
    //   }

    //   // increment burned balances
    //   if tx.output[vout].script_pubkey.is_op_return() {
    //     for (id, balance) in &balances {
    //       *burned.entry(*id).or_default() += balance;
    //     }
    //     continue;
    //   }

    //   buffer.clear();

    //   let mut balances = balances.into_iter().collect::<Vec<(u128, u128)>>();

    //   // Sort balances by id so tests can assert balances in a fixed order
    //   balances.sort();

    //   for (id, balance) in balances {
    //     varint::encode_to_vec(id, &mut buffer);
    //     varint::encode_to_vec(balance, &mut buffer);
    //   }

    //   self.outpoint_to_balances.insert(
    //     &OutPoint {
    //       txid,
    //       vout: vout.try_into().unwrap(),
    //     }
    //     .store(),
    //     buffer.as_slice(),
    //   )?;
    // }

    Ok(IndexRunesResult {
        output_dunes: allocated,
        successful_etch,
        successful_mints,
    })
}

fn create_rune_entry(
    chain_db: &ChainDB,
    db_tx: &Transaction<OptimisticTransactionDB>,
    artifact: &Dunestone,
    id: DuneId,
    dune: Dune,
    height: BlockHeight,
) -> Result<(), Error> {
    let kv_dune_id: storage::DuneId = id.into();

    // insert into (name -> ID) table
    DuneIdByNameKV::stage_upsert(
        &chain_db.db,
        DBUInt128(dune.0),
        DBSerde(kv_dune_id.clone()),
        db_tx,
    )?;

    let terms = if artifact.cenotaph {
        DuneTerms {
            name: dune.0,
            amount: None,
            cap: None,
            start_height: None,
            end_height: None,
        }
    } else {
        let Etching { terms, .. } = artifact.etching.unwrap();

        if let Some(terms) = terms {
            let amount = terms.limit;
            let cap = terms.cap;

            let relative_start = terms.offset.0.map(|offset| height.saturating_add(offset));

            let absolute_start = terms.height.0;

            let start = relative_start
                .zip(absolute_start)
                .map(|(relative, absolute)| relative.max(absolute))
                .or(relative_start)
                .or(absolute_start);

            let relative_end = terms.offset.1.map(|offset| height.saturating_add(offset));

            let absolute_end = terms.height.1;

            let end = relative_end
                .zip(absolute_end)
                .map(|(relative, absolute)| relative.min(absolute))
                .or(relative_end)
                .or(absolute_end);

            DuneTerms {
                name: dune.0,
                amount,
                cap,
                start_height: start,
                end_height: end,
            }
        } else {
            DuneTerms {
                name: dune.0,
                amount: None,
                cap: None,
                start_height: None,
                end_height: None,
            }
        }
    };

    DuneTermsByIdKV::stage_upsert(&chain_db.db, kv_dune_id, DBSerde(terms), db_tx)?;

    Ok(())
}

fn etched(
    chain_db: &ChainDB,
    db_tx: &Transaction<OptimisticTransactionDB>,
    _resolver: &HashMap<TxoRef, TxoBody>,
    tx_index: u32,
    _tx: &bitcoin::Transaction,
    artifact: &Dunestone,
    height: BlockHeight,
    network: BitcoinCompatibleNetwork,
) -> Result<Option<(DuneId, Dune)>, Error> {
    let dune = if let Some(x) = artifact.etching {
        x.dune
    } else {
        return Ok(None);
    };

    let chain = match network {
        BitcoinCompatibleNetwork::DogecoinTestnet | BitcoinCompatibleNetwork::BitcoinTestnet => {
            Chain::Testnet
        }
        _ => Chain::Mainnet,
    };
    let minimum = Dune::minimum_at_height(chain, Height(height as u32));

    let dune = if let Some(dune) = dune {
        if dune < minimum
            || dune.is_reserved()
            || DuneIdByNameKV::get_by_key(&chain_db.db, &db_tx, dune.0.into())?.is_some()
        {
            warn!("not etching {height}:{tx_index}: dune name");
            return Ok(None);
        }

        dune
    } else {
        let reserved_counter = DunesReservedCountersKV::get_by_key(
            &chain_db.db,
            &db_tx,
            DBBytes(DUNES_RESERVED_COUNTERS_KEY.to_vec()),
        )?
        .unwrap_or(DBInt(0));

        let new_counter = reserved_counter.0 + 1;

        DunesReservedCountersKV::stage_upsert(
            &chain_db.db,
            DBBytes(DUNES_RESERVED_COUNTERS_KEY.to_vec()),
            DBInt(new_counter),
            &db_tx,
        )?;

        Dune::reserved(reserved_counter.0.into())
    };

    Ok(Some((
        DuneId {
            height,
            index: tx_index,
        },
        dune,
    )))
}

fn mint(
    chain_db: &ChainDB,
    db_tx: &Transaction<OptimisticTransactionDB>,
    id: DuneId,
    height: BlockHeight,
) -> Result<Option<u128>, Error> {
    let id: storage::DuneId = id.into();

    let Some(DBSerde(terms)) = DuneTermsByIdKV::get_by_key(&chain_db.db, &db_tx, id.clone())?
    else {
        warn!("not minting {id:?}: no terms found");
        return Ok(None);
    };

    if let Some(start) = terms.start_height {
        if height < start {
            warn!("not minting {id:?}: start height");
            return Ok(None);
        }
    }

    if let Some(end) = terms.end_height {
        if height >= end {
            warn!("not minting {id:?}: end height");
            return Ok(None);
        }
    }

    let cap = terms.cap.unwrap_or(u128::MAX);

    let current_mints = DuneMintsByIdKV::get_by_key(&chain_db.db, db_tx, id.clone())?
        .map(|DBUInt128(x)| x)
        .unwrap_or_default();

    if current_mints >= cap {
        warn!("not minting {id:?}: cap reached {current_mints}/{cap}");
        return Ok(None);
    }

    // let new_mints = current_mints + 1;

    // RuneMintsByIdKV::stage_upsert(&chain_db.db, id, DBUInt128(new_mints), db_tx)?;

    Ok(Some(terms.amount.unwrap_or_default()))
}

fn unallocated(
    tx: &bitcoin::Transaction,
    resolver: &HashMap<TxoRef, TxoBody>,
) -> Result<HashMap<u128, u128>, Error> {
    // map of rune ID to un-allocated balance of that rune
    let mut unallocated: HashMap<u128, u128> = HashMap::new();

    // increment unallocated runes with the runes in tx inputs
    for input in tx.input.iter() {
        // skip coinbase input
        if !input.previous_output.is_null() {
            let outpoint = input.previous_output;

            let txo_ref = TxoRef(outpoint.txid, outpoint.vout);

            let txo_body = match resolver.get(&txo_ref) {
                Some(x) => x,
                None => {
                    error!(
                        "missing {txo_ref:?} for {:?} with resolver: {:?}",
                        tx.txid(),
                        resolver
                    );
                    return Err(Error::MissingTxo(
                        BlockHash::from_byte_array([0; 32]),
                        txo_ref.0,
                        txo_ref.1,
                    ));
                }
            };

            for (id, balance) in txo_body.dunes.iter() {
                let id = DuneId::from(id.clone());
                let id = id.clone().into();

                *unallocated.entry(id).or_default() += balance;
            }
        }
    }

    Ok(unallocated)
}
