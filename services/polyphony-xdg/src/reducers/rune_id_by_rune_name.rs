/*
   Rune Id by Rune Name
*/

use std::collections::{HashMap, HashSet};

use bitcoin::Block;
use serde::Deserialize;
use tracing::warn;

use crate::{dunes::dunestone::Dunestone, model};

use super::ReducerOutput;

#[derive(Deserialize)]
pub struct Config; // no config

pub struct Reducer;

#[derive(Clone, Debug)]
pub struct Output {
    pub rune_name: u128,
    pub rune_id: (u64, u32),
}

impl Reducer {
    pub fn reduce_block<'b>(
        &mut self,
        height: u64,
        block: &Block,
        ctx: &model::BlockContext,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        let rune_etch_tx_idxs = ctx
            .rune_etch_idxs
            .iter()
            .map(|x| x.0)
            .collect::<HashSet<_>>();

        let name_map = ctx
            .rune_etch_idxs
            .clone()
            .into_iter()
            .collect::<HashMap<_, _>>();

        // filter for txs with successful etchs (includes cenotaphs)
        for (idx, tx) in block
            .txdata
            .iter()
            .enumerate()
            .filter(|(idx, _)| rune_etch_tx_idxs.contains(&(*idx as u32)))
        {
            // try decode runestone
            let artifact = Dunestone::from_transaction(tx);

            match artifact {
                Some(Dunestone { etching, .. }) => match etching {
                    Some(_) => {
                        if let Some(n) = name_map.get(&(idx as u32)) {
                            outputs.push(ReducerOutput::RuneIdByRuneName(Output {
                                rune_id: (height, idx as u32),
                                rune_name: n.clone(),
                            }))
                        } else {
                            warn!(
                                "expected etching name for {} but didn't find one",
                                tx.compute_txid()
                            );
                        }
                    }
                    None => {
                        warn!(
                            "expected etching for {} but didn't find one",
                            tx.compute_txid()
                        );
                    }
                },
                None => {
                    warn!(
                        "expected artifact for {} but didn't find one",
                        tx.compute_txid()
                    );
                }
            }
        }

        Ok(())
    }
}

impl Config {
    pub fn plugin(self) -> super::Reducer {
        let reducer = Reducer;

        super::Reducer::RuneIdByRuneName(reducer)
    }
}
