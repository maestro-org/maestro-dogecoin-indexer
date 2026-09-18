/*
   Content by inscription ID
   Creates reducer outputs to store new inscriptions.
*/

use bitcoin::{hashes::Hash, Block};
use serde::Deserialize;

use crate::model;

use super::ReducerOutput;

#[derive(Deserialize)]
pub struct Config; // no config

pub struct Reducer;

#[derive(Clone, Debug)]
pub struct Output {
    // (reveal tx hash, index of inscriptions in reveal tx)
    pub inscription_id: ([u8; 32], u32),
    // block height of the reveal tx
    pub created_at: u64,
    // global inscription number
    pub inscription_num: u64,
    // type of the content body
    pub content_type: Vec<u8>,
    // inscription content body raw data
    pub content_body: Vec<u8>,
}

impl Reducer {
    pub fn reduce_block(
        &mut self,
        block_height: u64,
        block: &Block,
        ctx: &model::BlockContext,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        for tx in block.txdata.iter() {
            let txid = tx.compute_txid().to_byte_array();
            if let Some((inscription_id, inscription_num, content_type, content_body)) =
                ctx.new_inscriptions(&txid)
            {
                let inscription_id = (inscription_id.txid.to_byte_array(), inscription_id.index);
                outputs.push(ReducerOutput::ContentByInscriptionId(Output {
                    inscription_id,
                    created_at: block_height,
                    inscription_num,
                    content_type,
                    content_body,
                }))
            }
        }

        Ok(())
    }
}

impl Config {
    pub fn plugin(self) -> super::Reducer {
        let reducer = Reducer;

        super::Reducer::ContentByInscriptionId(reducer)
    }
}
