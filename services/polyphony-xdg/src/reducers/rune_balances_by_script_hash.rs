/*
   Rune Balances by Script Hash

   Creates reducer outputs which signal increases or decreases in total or
   available Dune balance for a script.

   Input utxo:
   - for every rune in the utxo, decrement the rune balance of the utxo owner

   Output utxo:
   - for every rune in the utxo, increment the rune balance of the utxo owner
*/

use bitcoin::{hashes::Hash, Block, OutPoint, TxOut};
use gasket::error::AsWorkError;
use serde::Deserialize;

use crate::{crosscut, model, prelude::AppliesPolicy};

use super::{IncrOrDecr, ReducerOutput};

#[derive(Deserialize)]
pub struct Config; // no config

pub struct Reducer {
    policy: crosscut::policies::RuntimePolicy,
}

#[derive(Clone, Debug)]
pub struct Output {
    pub script_hash: [u8; 20],
    pub rune_id: (u64, u32),
    pub delta: IncrOrDecr<u128>,
}

impl Reducer {
    fn process_consumed_txo(
        &mut self,
        ctx: &model::BlockContext,
        input: &OutPoint,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        let utxo = ctx.find_utxo(input).apply_policy(&self.policy).or_panic()?;

        let utxo = match utxo {
            Some(u) => u,
            None => return Ok(()),
        };

        if let Some(runes) = ctx.utxo_runes(&input) {
            for (rune_id, rune_amount) in runes {
                outputs.push(ReducerOutput::RuneBalancesByScriptHash(Output {
                    script_hash: utxo.txo.script_pubkey.script_hash().to_byte_array(),
                    rune_id: (rune_id.block, rune_id.tx),
                    delta: IncrOrDecr::Decrement(rune_amount),
                }))
            }
        }

        Ok(())
    }

    fn process_produced_txo(
        &mut self,
        ctx: &model::BlockContext,
        outpoint: OutPoint,
        tx_output: &TxOut,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        if let Some(runes) = ctx.utxo_runes(&outpoint) {
            let script_hash = tx_output.script_pubkey.script_hash();

            for (rune_id, rune_amount) in runes {
                outputs.push(ReducerOutput::RuneBalancesByScriptHash(Output {
                    script_hash: script_hash.to_byte_array(),
                    rune_id: (rune_id.block, rune_id.tx),
                    delta: IncrOrDecr::Increment(rune_amount),
                }));
            }
        }

        Ok(())
    }

    pub fn reduce_block<'b>(
        &mut self,
        _height: u64,
        block: &Block,
        ctx: &model::BlockContext,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        for tx in block.txdata.iter() {
            let txid = tx.compute_txid();

            // skip dummy input of coinbase tx
            if !tx.is_coinbase() {
                for txin in tx.input.iter().map(|x| x.previous_output) {
                    self.process_consumed_txo(&ctx, &txin, outputs)?;
                }
            }

            for (idx, txo) in tx.output.iter().enumerate() {
                let outpoint = OutPoint::new(txid, idx as u32);

                self.process_produced_txo(&ctx, outpoint, &txo, outputs)?;
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

        super::Reducer::RuneBalancesByScriptHash(reducer)
    }
}
