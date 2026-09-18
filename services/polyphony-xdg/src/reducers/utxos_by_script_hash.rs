/*
   UTxOs By Script Hash

   Creates reducer outputs to signal that a UTxO was consumed or produced, along
   with the hash of the script which controls it and the satoshis in the UTxO.
*/

use bitcoin::{hashes::Hash, Block, OutPoint, TxOut};
use serde::Deserialize;

use crate::{crosscut, model, prelude::*};

use super::{chained_txos, ReducerOutput, UtxoAction};

#[derive(Deserialize)]
pub struct Config; // no config

pub struct Reducer {
    policy: crosscut::policies::RuntimePolicy,
}

#[derive(Clone, Debug)]
pub struct Output {
    pub script_hash: [u8; 20], // hash of scriptbuf which controls utxo
    pub height: u64,
    pub utxo_hash: [u8; 32],
    pub utxo_index: u32,
    pub action: UtxoAction<u64>, // satoshis
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

        let script_hash = resolved_utxo.txo.script_pubkey.script_hash();

        outputs.push(ReducerOutput::UtxosByScriptHash(Output {
            script_hash: script_hash.to_byte_array(),
            height: resolved_utxo.height,
            utxo_hash: input.txid.to_byte_array(),
            utxo_index: input.vout,
            action: UtxoAction::Consumed,
        }));

        Ok(())
    }

    fn process_produced_txo(
        &mut self,
        outpoint: OutPoint,
        tx_output: &TxOut,
        outputs: &mut Vec<ReducerOutput>,
        height: u64,
    ) -> Result<(), gasket::error::Error> {
        let script_hash = tx_output.script_pubkey.script_hash();

        outputs.push(ReducerOutput::UtxosByScriptHash(Output {
            script_hash: script_hash.to_byte_array(),
            height,
            utxo_hash: outpoint.txid.to_byte_array(),
            utxo_index: outpoint.vout,
            action: UtxoAction::Produced(tx_output.value.to_sat()),
        }));

        Ok(())
    }

    pub fn reduce_block<'b>(
        &mut self,
        height: u64,
        block: &Block,
        ctx: &model::BlockContext,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        let (txids, chained_txos) = chained_txos(&block.txdata);

        for (tx, txid) in block.txdata.iter().zip(txids) {
            // skip dummy input of coinbase tx
            if !tx.is_coinbase() {
                for txin in tx
                    .input
                    .iter()
                    .map(|x| x.previous_output)
                    .filter(|x| !chained_txos.contains(&x))
                {
                    self.process_consumed_txo(&ctx, &txin, outputs)?;
                }
            }

            for (idx, txo) in tx.output.iter().enumerate() {
                let outpoint = OutPoint::new(txid, idx as u32);

                if !chained_txos.contains(&outpoint) {
                    self.process_produced_txo(outpoint, txo, outputs, height)?;
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

        super::Reducer::UtxosByScriptHash(reducer)
    }
}
