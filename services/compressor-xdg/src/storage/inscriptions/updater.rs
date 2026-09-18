use std::{cmp::min, collections::HashMap};

use bitcoin::{
    consensus::{Decodable, Encodable},
    hashes::Hash,
    BlockHash, OutPoint, ScriptHash, TxOut, Txid,
};
use ordinals::{Height, SatPoint};
use rocksdb::{OptimisticTransactionDB, Transaction};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::storage::{
    BlockHeight, ChainDB, DBBytes, DBHash32, DBSerde, DBUInt128, Error, KVTable,
    NewInscriptionInfo, TxoBody, TxoRef,
};

use super::{
    brc20::{self, DeployAction, MintAction, TransferAction},
    inscription::{Inscription, ParsedInscription},
    inscription_id::InscriptionId,
    BRC20Balances, BRC20Terms, Counters, PartialTxidToBytesKV, PartialTxidToTxidsKV,
    ScriptAndBRC20Kind, SupplyByBRC20, TermsByBRC20Ticker,
};

#[derive(Debug, Clone)]
pub struct Flotsam {
    inscription_id: InscriptionId,
    offset: u64,
    origin: Origin,
}

#[derive(Debug, Clone)]
enum Origin {
    New { fee: u64 },
    Old { old_satpoint: SatPoint },
}

pub struct IndexInscriptionsResult {
    // for each output, vec of inscriptions and their offsets in the output
    pub output_inscriptions: Vec<Vec<(u64, InscriptionId)>>,
    // for each output, vec of inscriptions and their offsets in the output
    pub lost_or_unbound_inscriptions: Vec<(SatPoint, InscriptionId)>,
    // reinscriptions indexes which were allowed
    pub valid_reinscriptions: Vec<u32>,
    pub brc20_resolver: Vec<(InscriptionId, BRC20Message)>,
    pub new_unused_brc20_transfers: HashMap<InscriptionId, (Vec<u8>, u128)>,
}

pub fn index_inscriptions(
    resolver: &HashMap<TxoRef, TxoBody>,
    chain_db: &ChainDB,
    db_tx: &Transaction<OptimisticTransactionDB>,
    tx: &bitcoin::Transaction,
    txid: Txid,
    height: BlockHeight,
    _jubilee_height: BlockHeight,
    counters: &mut Counters,
    reward: &mut u64,
    mut cb_inscriptions: &mut Vec<Flotsam>,
    new_inscriptions: &mut Vec<NewInscriptionInfo>,
) -> Result<IndexInscriptionsResult, Error> {
    let mut outgoing_inscriptions = Vec::new();

    let mut outpoint_to_script = HashMap::with_capacity(tx.output.len());
    let mut brc_20_inscriptions = Vec::new();
    let mut unused_brc20_transfers = HashMap::new();
    let mut new_unused_brc20_transfers = HashMap::new();
    let mut valid_brc20s = Vec::new();

    let mut floating_inscriptions = Vec::new();
    // let mut id_counter = 0;
    let mut total_input_value = 0;
    let total_output_value = tx.output.iter().map(|txout| txout.value).sum::<u64>();

    // process inputs, adding inscriptions contained in the inputs and new
    // inscriptions to the floating inscriptions
    for (_, txin) in tx.input.iter().enumerate() {
        // skip subsidy since no inscriptions possible
        if txin.previous_output.is_null() {
            // Known limitation: Bitcoin subsidy schedule (see storage/txos)
            total_input_value += Height(height as u32).subsidy();
            continue;
        }

        let txo_ref = TxoRef(txin.previous_output.txid, txin.previous_output.vout as u32);

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

        let txout = TxOut::consensus_decode(&mut &txo_body.raw[..]).unwrap();

        for (offset, inscription_id) in txo_body.inscriptions.clone() {
            let old_satpoint = SatPoint {
                outpoint: txin.previous_output,
                offset,
            };

            let offset = total_input_value + offset;
            floating_inscriptions.push(Flotsam {
                offset,
                inscription_id,
                origin: Origin::Old { old_satpoint },
            });

            if let Some((tick, amt)) = txo_body.unused_brc20_transfers.get(&inscription_id) {
                unused_brc20_transfers.insert(
                    inscription_id,
                    (tick, amt, txout.script_pubkey.script_hash(), old_satpoint),
                );
            }
        }

        total_input_value += txout.value;
    }

    // if none of the inscriptions in the inputs are at offset 0, try process
    // any which are partial
    if floating_inscriptions
        .iter()
        .all(|flotsam| flotsam.offset != 0)
    {
        // the first input is the output which will have a partial inscription
        let previous_txid = tx.input[0].previous_output.txid;
        let previous_txid_bytes = DBHash32(previous_txid.to_byte_array());
        let mut txids_vec = vec![];

        let maybe_txids =
            PartialTxidToTxidsKV::get_by_key(&chain_db.db, db_tx, previous_txid_bytes.clone())?;

        // if the first input has a partial inscription, get the previous transactions
        // else intialise with the current tx
        let txs = match maybe_txids {
            Some(DBBytes(txids)) => {
                debug!("tx {txid} has previous partials: {txids:?}");

                let mut txs = vec![];
                txids_vec = txids.to_vec();
                for i in 0..txids.len() / 32 {
                    let txid: &[u8; 32] = &txids[i * 32..i * 32 + 32].try_into().unwrap();

                    let DBBytes(tx_buf) =
                        PartialTxidToBytesKV::get_by_key(&chain_db.db, db_tx, DBHash32(*txid))?
                            .unwrap();

                    let mut cursor = std::io::Cursor::new(tx_buf);
                    let tx = bitcoin::Transaction::consensus_decode(&mut cursor)
                        .map_err(Error::BitcoinDecode)?;
                    txs.push(tx);
                }
                txs.push(tx.clone());
                txs
            }
            None => {
                vec![tx.clone()]
            }
        };

        match Inscription::from_transactions(txs) {
            ParsedInscription::None => {}
            // if we can build a partial inscriptions from the inscriptions then
            ParsedInscription::Partial => {
                // add the current txid to the txids vec
                let mut txid_vec = txid.to_byte_array().to_vec();
                txids_vec.append(&mut txid_vec);

                PartialTxidToTxidsKV::stage_upsert(
                    &chain_db.db,
                    DBHash32(txid.to_byte_array()),
                    DBBytes(txids_vec),
                    db_tx,
                )?;

                let mut tx_buf = vec![];
                tx.consensus_encode(&mut tx_buf)
                    .map_err(Error::BitcoinEncode)?;

                // self
                //   .txid_to_tx
                //   .insert(&txid.into_inner().as_slice(), tx_buf.as_slice())?;

                PartialTxidToBytesKV::stage_upsert(
                    &chain_db.db,
                    DBHash32(txid.to_byte_array()),
                    DBBytes(tx_buf),
                    db_tx,
                )?;
            }

            ParsedInscription::Complete(inscription) => {
                let mut txid_vec = txid.to_byte_array().to_vec();
                txids_vec.append(&mut txid_vec);

                // also add the inscription to the floating inscriptions
                let og_inscription_id = InscriptionId {
                    txid: Txid::from_slice(&txids_vec[0..32]).unwrap(),
                    index: 0,
                };

                floating_inscriptions.push(Flotsam {
                    inscription_id: og_inscription_id,
                    offset: 0,
                    origin: Origin::New {
                        fee: total_input_value - total_output_value,
                    },
                });

                if let Some(m) = brc20::unparsed_message(&inscription) {
                    debug!("found brc20 message: {txid:?} {og_inscription_id:?} {m:?}");
                    brc_20_inscriptions.push((og_inscription_id, m))
                }

                // add new inscription for gRPC messages
                new_inscriptions.push(NewInscriptionInfo {
                    tx_hash: txid.to_byte_array(), // hash of reveal tx
                    id: og_inscription_id,
                    number: counters.next_sequence_num, // every Dogecoin tx may have at most 1 inscription
                    body: inscription.body().unwrap_or_default().to_vec(), // content body raw data
                    body_type: inscription.content_type().unwrap_or_default().into(), // content body type
                });
            }
        }
    };

    let is_coinbase = tx
        .input
        .first()
        .map(|tx_in| tx_in.previous_output.is_null())
        .unwrap_or_default();

    if is_coinbase {
        floating_inscriptions.append(&mut cb_inscriptions);
    }

    floating_inscriptions.sort_by_key(|flotsam| flotsam.offset);
    let mut inscriptions = floating_inscriptions.into_iter().peekable();

    let mut new_locations = Vec::new();
    let mut output_value = 0;
    for (vout, tx_out) in tx.output.iter().enumerate() {
        outpoint_to_script.insert(
            OutPoint {
                txid,
                vout: vout as u32,
            },
            tx_out.script_pubkey.script_hash(),
        );

        let end = output_value + tx_out.value;

        while let Some(flotsam) = inscriptions.peek() {
            if flotsam.offset >= end {
                break;
            }

            let new_satpoint = SatPoint {
                outpoint: OutPoint {
                    txid,
                    vout: vout.try_into().unwrap(),
                },
                offset: flotsam.offset - output_value,
            };

            new_locations.push((new_satpoint, inscriptions.next().unwrap()));
        }

        output_value = end;
    }

    for (new_satpoint, flotsam) in new_locations.into_iter() {
        update_inscription_location(
            chain_db,
            db_tx,
            flotsam,
            new_satpoint,
            counters,
            &mut outgoing_inscriptions,
        )?;
    }

    if is_coinbase {
        for flotsam in inscriptions {
            let new_satpoint = SatPoint {
                outpoint: OutPoint::null(),
                offset: counters.lost_sats + flotsam.offset - output_value,
            };

            update_inscription_location(
                chain_db,
                db_tx,
                flotsam,
                new_satpoint,
                counters,
                &mut outgoing_inscriptions,
            )?;
        }

        counters.lost_sats += *reward - output_value;
    } else {
        // if unused brc20 transfer inscription lost as fee, return brc20 to sender
        for flotsam in inscriptions.clone() {
            if let Some((tick, amt, sender, original_point)) =
                unused_brc20_transfers.get(&flotsam.inscription_id)
            {
                let receiver = sender;

                info!(
                    "found unused brc20transfer as fee {:?} {}",
                    &(tick, amt, sender, original_point),
                    hex::encode(receiver.to_byte_array())
                );

                let balance_key = DBSerde(ScriptAndBRC20Kind {
                    script: receiver.to_byte_array(),
                    brc20_ticker: tick.to_vec(),
                });

                let old_balance =
                    BRC20Balances::get_by_key(&chain_db.db, db_tx, balance_key.clone())?
                        .map(|DBUInt128(x)| x)
                        .unwrap_or_default();

                let new_balance = old_balance + **amt;

                BRC20Balances::stage_upsert(
                    &chain_db.db,
                    balance_key,
                    DBUInt128(new_balance),
                    db_tx,
                )?;

                valid_brc20s.push((
                    flotsam.inscription_id.clone(),
                    BRC20Message::Transfer(
                        tick.to_vec(),
                        **amt,
                        original_point.outpoint.clone(),
                        *receiver,
                    ),
                ))
            }
        }

        cb_inscriptions.extend(inscriptions.map(|flotsam| Flotsam {
            offset: *reward + flotsam.offset - output_value,
            ..flotsam
        }));

        *reward += total_input_value - output_value;
    }

    let mut inscription_id_to_satpoint = HashMap::new();
    let mut inscription_id_to_script = HashMap::new();

    for (satpoint, inscid) in outgoing_inscriptions.iter() {
        inscription_id_to_satpoint.insert(inscid, satpoint);

        // could be outgoing to unbound or miner
        if let Some(script) = outpoint_to_script.get(&satpoint.outpoint) {
            inscription_id_to_script.insert(inscid, script);
        }

        // if the inscription is an unused transfer, increase available balance of receiver.
        // we already handle the lost-as-fee case (should not be in outgoing inscriptions
        // unless in cb tx, but in that case it will not be in unused brc20 transfers)
        if let Some((tick, amt, sender, original_point)) = unused_brc20_transfers.get(inscid) {
            let receiver = if satpoint.outpoint.is_null() {
                warn!("unexpected null outpoint for brc20 transfer {:?}", inscid);
                sender // return to sender as inscription lost as fee
            } else {
                outpoint_to_script.get(&satpoint.outpoint).unwrap() // send to inscription receiver
            };

            let balance_key = DBSerde(ScriptAndBRC20Kind {
                script: receiver.to_byte_array(),
                brc20_ticker: tick.to_vec(),
            });

            let old_balance = BRC20Balances::get_by_key(&chain_db.db, db_tx, balance_key.clone())?
                .map(|DBUInt128(x)| x)
                .unwrap_or_default();

            let new_balance = old_balance + **amt;

            BRC20Balances::stage_upsert(&chain_db.db, balance_key, DBUInt128(new_balance), db_tx)?;

            valid_brc20s.push((
                inscid.clone(),
                BRC20Message::Transfer(
                    tick.to_vec(),
                    **amt,
                    original_point.outpoint.clone(),
                    *receiver,
                ),
            ))
        }
    }

    // for each new brc_20 message
    for (insc, brc20_msg) in brc_20_inscriptions {
        debug!("processing brc20 message {insc:?} {brc20_msg:?}");
        // fetch terms for ticker

        let terms = TermsByBRC20Ticker::get_by_key(
            &chain_db.db,
            db_tx,
            DBBytes(brc20_msg.tick.to_lowercase().as_bytes().to_vec()),
        )?
        .map(|DBSerde(x)| x);

        // deploy:
        //      invalid if terms found, valid if not
        //      effects:
        //          insert terms into database

        if let Some(deploy) = DeployAction::parse(&brc20_msg) {
            if terms.is_none() {
                let new_terms = BRC20Terms {
                    max: deploy.max,
                    mint_amt_limit: deploy.limit,
                    dec: deploy.dec,
                    self_mint: deploy.self_mint,
                    deploy_id: insc,
                };

                TermsByBRC20Ticker::stage_upsert(
                    &chain_db.db,
                    DBBytes(deploy.ticker.clone()),
                    DBSerde(new_terms),
                    db_tx,
                )?;

                valid_brc20s.push((insc, BRC20Message::Deploy(deploy.ticker)));
            } else {
                warn!("rejecting brc20: deploy when already deployed");
            }

            // next brc 20 msg
            continue;
        }

        // mint:
        //      invalid if no terms found (OK)
        //      invalid if amt > lim (OK)
        //      invalid if supply = max (OK)
        //      invalid if sent as fee ? (we dont see unbound here)
        //      invalid if self mint and deploy inscription not in tx possible parents (OK)
        //      effects:
        //          increase balance of receiver by min(max-supply, amt) (OK)
        //          increase token supply by min(max-supply, amt) (OK)

        if let Some(terms) = terms {
            if let Some(mut mint) = MintAction::parse(&brc20_msg, &terms) {
                let supply =
                    SupplyByBRC20::get_by_key(&chain_db.db, db_tx, DBBytes(mint.ticker.clone()))?
                        .map(|DBUInt128(x)| x)
                        .unwrap_or_default();

                let receiver = if let Some(x) = inscription_id_to_script.get(&insc) {
                    x
                } else {
                    warn!("rejecting brc20: mint to non-output");
                    continue;
                };

                if supply == terms.max {
                    warn!("rejecting brc20: supply max reached {receiver}");
                    continue;
                }

                let actual_mint_amount = min(mint.amt, terms.max - supply);

                mint.amt = actual_mint_amount;

                // increase total minted supply

                let new_supply = supply + mint.amt;

                SupplyByBRC20::stage_upsert(
                    &chain_db.db,
                    DBBytes(mint.ticker.clone()),
                    DBUInt128(new_supply),
                    db_tx,
                )?;

                // update receiver balance

                let balance_key = DBSerde(ScriptAndBRC20Kind {
                    script: receiver.to_byte_array(),
                    brc20_ticker: mint.ticker.clone(),
                });

                let old_balance =
                    BRC20Balances::get_by_key(&chain_db.db, db_tx, balance_key.clone())?
                        .map(|DBUInt128(x)| x)
                        .unwrap_or_default();

                let new_balance = old_balance + mint.amt;

                BRC20Balances::stage_upsert(
                    &chain_db.db,
                    balance_key,
                    DBUInt128(new_balance),
                    db_tx,
                )?;

                valid_brc20s.push((insc, BRC20Message::Mint(mint.ticker, mint.amt, **receiver)));

                // next brc 20 msg
                continue;
            }

            // transfer:
            //      invalid if no terms found
            //      invalid if available balance of receiver less than transfer amount
            //      effects:
            //          decrease available balance of receiver (it is now locked in the transfer inscription)

            if let Some(transfer) = TransferAction::parse(&brc20_msg, &terms) {
                // decrease receiver balance

                let receiver = if let Some(x) = inscription_id_to_script.get(&insc) {
                    x
                } else {
                    warn!("rejecting brc20: transfer init to non-output");
                    continue;
                };

                let balance_key = DBSerde(ScriptAndBRC20Kind {
                    script: receiver.to_byte_array(),
                    brc20_ticker: transfer.ticker.clone(),
                });

                let old_balance = if let Some(bal) =
                    BRC20Balances::get_by_key(&chain_db.db, db_tx, balance_key.clone())?
                        .map(|DBUInt128(x)| x)
                {
                    bal
                } else {
                    warn!("rejecting brc20: no balance {receiver}");
                    continue;
                };

                if old_balance < transfer.amt {
                    warn!("rejecting brc20: trying to transfer more than balance {old_balance}, {receiver}");
                    continue;
                }

                let new_balance = old_balance - transfer.amt;

                BRC20Balances::stage_upsert(
                    &chain_db.db,
                    balance_key,
                    DBUInt128(new_balance),
                    db_tx,
                )?;

                new_unused_brc20_transfers.insert(insc, (transfer.ticker.clone(), transfer.amt));

                valid_brc20s.push((
                    insc,
                    BRC20Message::TransferInit(transfer.ticker, transfer.amt, **receiver),
                ));

                // next brc 20 msg
                continue;
            }
        } else {
            let receiver = inscription_id_to_script.get(&insc);

            warn!("rejecting brc20: not deploy but no terms {receiver:?}")
        }
    }

    let (contained, not_contained): (Vec<_>, Vec<_>) = outgoing_inscriptions
        .into_iter()
        .partition(|x| x.0.outpoint.txid == txid);

    let mut output_inscriptions = vec![Vec::new(); tx.output.len()];

    for (satpoint, inscription) in contained {
        let output_index = satpoint.outpoint.vout;

        output_inscriptions
            .get_mut(output_index as usize)
            .unwrap()
            .push((satpoint.offset, inscription));
    }

    if output_inscriptions.iter().any(|x| !x.is_empty()) {
        debug!("oi {:?}", output_inscriptions);
    }

    Ok(IndexInscriptionsResult {
        output_inscriptions,
        lost_or_unbound_inscriptions: not_contained,
        valid_reinscriptions: vec![], // not supported
        brc20_resolver: valid_brc20s,
        new_unused_brc20_transfers,
    })
}

fn update_inscription_location(
    _chain_db: &ChainDB,
    _db_tx: &Transaction<OptimisticTransactionDB>,
    flotsam: Flotsam,
    new_satpoint: SatPoint,
    counters: &mut Counters,
    output_inscriptions: &mut Vec<(SatPoint, InscriptionId)>,
) -> Result<(), Error> {
    let inscription_id = flotsam.inscription_id;

    match flotsam.origin {
        Origin::Old { .. } => (),
        Origin::New { .. } => {
            // increment global counters
            debug!(
                "new inscription: {:?} : {} : {} : {:?}",
                inscription_id, counters.blessed_count, counters.next_sequence_num, new_satpoint
            );

            counters.blessed_count += 1;

            counters.next_sequence_num += 1;
        }
    }

    output_inscriptions.push((new_satpoint, inscription_id));

    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BRC20Message {
    Deploy(Vec<u8>),
    Mint(Vec<u8>, u128, ScriptHash),
    Transfer(Vec<u8>, u128, OutPoint, ScriptHash),
    TransferInit(Vec<u8>, u128, ScriptHash),
}
