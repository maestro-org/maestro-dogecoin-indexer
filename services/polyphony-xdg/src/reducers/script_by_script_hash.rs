/*
   Script by Script Hash

   Creates reducer output for every output which maps a script hash to the
   script pubkey preimage. This allows us to store script hash instead of the
   full in most places instead of duplicating potentially large scripts, then
   we can resolve the actual script if needed.
*/

use bitcoin::{hashes::Hash, Block, TxOut};
use serde::Deserialize;

use crate::model;

use super::ReducerOutput;

#[derive(Deserialize)]
pub struct Config; // no config

pub struct Reducer;

#[derive(Clone, Debug)]
pub struct Output {
    pub script_hash: [u8; 20],
    pub script: Vec<u8>,
}

impl Reducer {
    fn process_produced_txo(
        &mut self,
        tx_output: &TxOut,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        let script_hash = tx_output.script_pubkey.script_hash();
        let script = tx_output.script_pubkey.to_bytes();

        outputs.push(ReducerOutput::ScriptByScriptHash(Output {
            script_hash: script_hash.to_byte_array(),
            script,
        }));

        Ok(())
    }

    pub fn reduce_block<'b>(
        &mut self,
        _height: u64,
        block: &Block,
        _ctx: &model::BlockContext,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        for tx in block.txdata.iter() {
            for txo in tx.output.iter() {
                self.process_produced_txo(txo, outputs)?;
            }
        }

        Ok(())
    }
}

impl Config {
    pub fn plugin(self) -> super::Reducer {
        let reducer = Reducer;

        super::Reducer::ScriptByScriptHash(reducer)
    }
}
