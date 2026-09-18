/*
   Inscriptions by UTxO

   Creates reducer outputs to signal that a UTxO containing some inscriptions
   was consumed or produced, along with the inscription IDs and their offsets.
*/

use bitcoin::{hashes::Hash, Block, OutPoint};
use serde::Deserialize;

use crate::model;

use super::{ReducerOutput, UtxoAction};

#[derive(Deserialize)]
pub struct Config; // no config

pub struct Reducer;

#[derive(Clone, Debug)]
pub struct Output {
    pub utxo_hash: [u8; 32],
    pub utxo_index: u32,
    pub action: UtxoAction<Vec<(u32, ([u8; 32], u32))>>, // list of (offset, (id_txhash, id_index))
}

impl Reducer {
    fn process_consumed_txo(
        &mut self,
        ctx: &model::BlockContext,
        input: &OutPoint,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        if ctx.utxo_inscriptions(input).is_some() {
            outputs.push(ReducerOutput::InscriptionsByUtxo(Output {
                utxo_hash: input.txid.to_byte_array(),
                utxo_index: input.vout,
                action: UtxoAction::Consumed,
            }))
        }

        Ok(())
    }

    fn process_produced_txo(
        &mut self,
        ctx: &model::BlockContext,
        outpoint: OutPoint,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        if let Some(inscriptions) = ctx.utxo_inscriptions(&outpoint) {
            let mut inscriptions: Vec<_> = inscriptions
                .into_iter()
                .map(|(offset, id)| (offset, (id.txid.to_byte_array(), id.index)))
                .collect();

            inscriptions.truncate(10000);

            outputs.push(ReducerOutput::InscriptionsByUtxo(Output {
                utxo_hash: outpoint.txid.to_byte_array(),
                utxo_index: outpoint.vout,
                action: UtxoAction::Produced(inscriptions),
            }))
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

            for (idx, _) in tx.output.iter().enumerate() {
                let outpoint = OutPoint::new(txid, idx as u32);

                self.process_produced_txo(&ctx, outpoint, outputs)?;
            }
        }

        Ok(())
    }
}

impl Config {
    pub fn plugin(self) -> super::Reducer {
        let reducer = Reducer;

        super::Reducer::InscriptionsByUtxo(reducer)
    }
}
