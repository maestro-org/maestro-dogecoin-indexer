/*
    Transfer inscriptions by script hash

    Creates reducer outputs to signal that a transfer inscription was consumed or produced. When
    produced, we additionally store:
        - number of BRC20 tokens locked in the transfer inscription,
        - number of sats locked in the transfer inscription,
        - the UTxO ref containing the inscribed sat,
        - the offset of the inscribed sat in this UTxO, and
        - the block height of this UTxO.
*/

use bitcoin::{hashes::Hash, Block, OutPoint};
use serde::Deserialize;

use crate::{crosscut, inscriptions::inscription_id::InscriptionId, model, prelude::*};

use super::{ReducerOutput, UtxoAction};

#[derive(Deserialize)]
pub struct Config; // no config

pub struct Reducer {
    policy: crosscut::policies::RuntimePolicy,
}

#[derive(Clone, Debug)]
pub struct Output {
    pub script_hash: [u8; 20],
    // UTF-8 encoding of the inscription ticker.
    pub ticker: Vec<u8>,
    // Inscription ID: (hash of reveal tx, index of new inscription in reveal tx).
    pub inscription_id: ([u8; 32], u32),
    // (
    //      token amount locked in the transfer inscription,
    //      sat amount locked in the transfer inscription,
    //      UTxO ref,
    //      sat offset in UTxO,
    //      block height,
    // )
    pub action: UtxoAction<(u128, u64, ([u8; 32], u32), u32, u64)>,
}

impl Reducer {
    fn process_consumed_txo(
        &mut self,
        ctx: &model::BlockContext,
        input: &OutPoint,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        for (_, id) in ctx.utxo_inscriptions(input).unwrap_or_default() {
            for action in ctx.inscription_brc20s(&id).unwrap_or_default() {
                if let model::BRC20Message::Transfer(ticker, _, initial_utxo, _) = action {
                    // check whether this is the first time the inscription is spent
                    if *input == initial_utxo {
                        let resolved_utxo =
                            ctx.find_utxo(input).apply_policy(&self.policy).or_panic()?;
                        let resolved_utxo = match resolved_utxo {
                            Some(u) => u,
                            None => return Ok(()),
                        };
                        let script_hash = resolved_utxo
                            .txo
                            .script_pubkey
                            .script_hash()
                            .to_byte_array();
                        let inscription_id = (id.txid.to_byte_array(), id.index);

                        outputs.push(ReducerOutput::TransferInscriptionsByScriptHash(Output {
                            script_hash,
                            ticker,
                            inscription_id,
                            action: UtxoAction::Consumed,
                        }))
                    }
                }
            }
        }

        Ok(())
    }

    pub fn reduce_block(
        &mut self,
        height: u64,
        block: &Block,
        ctx: &model::BlockContext,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        for tx in block.txdata.iter() {
            let txid = tx.compute_txid();

            // process inputs
            // skip dummy input of coinbase tx
            if !tx.is_coinbase() {
                for txin in tx.input.iter().map(|x| x.previous_output) {
                    self.process_consumed_txo(ctx, &txin, outputs)?;
                }
            }

            // process outputs
            // in Dogecoin, transactions can have at most one inscription
            let inscription_id: InscriptionId = InscriptionId { txid, index: 0 };
            for action in ctx.inscription_brc20s(&inscription_id).unwrap_or_default() {
                // we only store data for the initialization of the transfer inscription
                if let model::BRC20Message::TransferInit(ticker, amt, receiver) = action {
                    // search for the output containing this inscription
                    'find_output: for (txout_idx, txout) in tx.output.iter().enumerate() {
                        let outpoint = &OutPoint::new(txid, txout_idx as u32);
                        for (offset, id) in ctx.utxo_inscriptions(outpoint).unwrap_or_default() {
                            if id == inscription_id {
                                outputs.push(ReducerOutput::TransferInscriptionsByScriptHash(
                                    Output {
                                        script_hash: receiver.to_byte_array(),
                                        ticker: ticker.clone(),
                                        inscription_id: (txid.to_byte_array(), 0),
                                        action: UtxoAction::Produced((
                                            amt,
                                            txout.value.to_sat(),
                                            (txid.to_byte_array(), txout_idx as u32),
                                            offset,
                                            height,
                                        )),
                                    },
                                ));
                                break 'find_output;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl Config {
    pub fn plugin(self, policy: &crosscut::policies::RuntimePolicy) -> super::Reducer {
        let reducer = Reducer {
            policy: policy.clone(),
        };

        super::Reducer::TransferInscriptionsByScriptHash(reducer)
    }
}
