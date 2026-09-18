/*
   Mints by Rune Id

   Creates reducer output which signals that a transaction contained a Runestone
   which minted a Rune, so we can increment a total mints by rune ID counter.
*/

use std::collections::HashSet;

use bitcoin::Block;
use serde::Deserialize;

use crate::{
    dunes::{claim, dune_id::DuneId, dunestone::Dunestone},
    model,
};

use super::ReducerOutput;

#[derive(Deserialize)]
pub struct Config; // no config

pub struct Reducer;

#[derive(Clone, Debug)]
pub struct Output {
    pub rune_id: (u64, u32),
}

impl Reducer {
    pub fn reduce_block<'b>(
        &mut self,
        _height: u64,
        block: &Block,
        ctx: &model::BlockContext,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        let txs_with_mints: HashSet<u32> = ctx.rune_mint_idxs.iter().map(|x| x.0).collect();

        // filter for txs with successful mints (meaning OK according to terms), including cenotaphs
        for (txidx, tx) in block
            .txdata
            .iter()
            .enumerate()
            .filter(|(idx, _)| txs_with_mints.contains(&(*idx as u32)))
        {
            // try decode runestone
            let artifact = Dunestone::from_transaction(tx);

            if let Some(dunestone) = artifact {
                if !dunestone.cenotaph {
                    let mut claims = dunestone
                        .edicts
                        .iter()
                        .filter_map(|edict| claim(edict.id))
                        .collect::<Vec<u128>>();
                    claims.sort();
                    claims.dedup();

                    for (edict_idx, id) in claims.into_iter().enumerate() {
                        if let Ok(key) = DuneId::try_from(id) {
                            if ctx
                                .rune_mint_idxs
                                .contains(&(txidx as u32, edict_idx as u32))
                            {
                                outputs.push(ReducerOutput::MintsByRuneId(Output {
                                    rune_id: (key.height, key.index),
                                }))
                            } else {
                                continue;
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
    pub fn plugin(self) -> super::Reducer {
        let reducer = Reducer;

        super::Reducer::MintsByRuneId(reducer)
    }
}
