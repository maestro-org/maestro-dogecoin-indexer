/*
   Inscriptions by script hash
   Creates reducer outputs to signal that a script containing some inscriptions was consumed or
   produced (along with the UTxO ref containing the inscribed sat, the block height of this UTxO,
   and offset in the UTxO of the inscribed sat).
*/

use bitcoin::{hashes::Hash, Block, OutPoint, TxOut};
use serde::Deserialize;

use crate::{crosscut, model, prelude::*};

use super::{ReducerOutput, UtxoAction};

#[derive(Deserialize)]
pub struct Config; // no config

pub struct Reducer {
    policy: crosscut::policies::RuntimePolicy,
}

#[derive(Clone, Debug)]
pub struct Output {
    pub script_hash: [u8; 20],
    // inscription ID: hash of reveal tx, index of inscription in reveal tx
    pub inscription_id: ([u8; 32], u32),
    // (block height, UTxO ref, sat offset in UTxO), where UTxO ref: (tx hash, output index)
    pub action: UtxoAction<(u64, ([u8; 32], u32), u32)>,
}

impl Reducer {
    fn process_consumed_txo(
        &mut self,
        ctx: &model::BlockContext,
        input: &OutPoint,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        let resolved_utxo = ctx.find_utxo(input).apply_policy(&self.policy).or_panic()?;

        let resolved_utxo = match resolved_utxo {
            Some(u) => u,
            None => return Ok(()),
        };

        let script_hash = resolved_utxo
            .txo
            .script_pubkey
            .script_hash()
            .to_byte_array();

        if let Some(inscriptions) = ctx.utxo_inscriptions(input) {
            for (_, id) in inscriptions {
                let inscription_id = (id.txid.to_byte_array(), id.index);
                outputs.push(ReducerOutput::InscriptionsByScriptHash(Output {
                    script_hash,
                    inscription_id,
                    action: UtxoAction::Consumed,
                }))
            }
        }

        Ok(())
    }

    fn process_produced_txo(
        &mut self,
        ctx: &model::BlockContext,
        height: u64,
        output: &OutPoint,
        tx_output: &TxOut,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        let script_hash = tx_output.script_pubkey.script_hash().to_byte_array();

        if let Some(inscriptions) = ctx.utxo_inscriptions(output) {
            for (offset, id) in inscriptions.into_iter() {
                let inscription_id = (id.txid.to_byte_array(), id.index);
                outputs.push(ReducerOutput::InscriptionsByScriptHash(Output {
                    script_hash,
                    inscription_id,
                    action: UtxoAction::Produced((
                        height,
                        (output.txid.to_byte_array(), output.vout),
                        offset,
                    )),
                }))
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

            // skip dummy input of coinbase tx
            if !tx.is_coinbase() {
                for txin in tx.input.iter().map(|x| x.previous_output) {
                    self.process_consumed_txo(ctx, &txin, outputs)?;
                }
            }

            for (idx, txout) in tx.output.iter().enumerate() {
                let outpoint = &OutPoint::new(txid, idx as u32);

                self.process_produced_txo(ctx, height, outpoint, txout, outputs)?;
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

        super::Reducer::InscriptionsByScriptHash(reducer)
    }
}
