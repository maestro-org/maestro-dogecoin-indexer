/*
   Etching by Rune Id

   Creates reducer output containing details of the etching for each Rune Id
*/

use std::collections::{HashMap, HashSet};

use bitcoin::{hashes::Hash, Block};
use serde::Deserialize;
use tracing::warn;

use crate::{dunes::dunestone::Dunestone, model};

use super::ReducerOutput;

#[derive(Deserialize)]
pub struct Config; // no config

pub struct Reducer;

#[derive(Debug, Clone)]
pub struct EtchingInfo {
    pub name: Option<u128>,
    pub spacers: Option<u32>,
    pub symbol: Option<u32>, // char as u32
    pub divisibility: Option<u8>,
    // pub premine: Option<u128>, // TODO: not explicit in Dunes, unlike Runes
    pub max_mint_txs: Option<u128>,
    pub amount_per_mint: Option<u128>,
    pub start_height: Option<u64>,
    pub end_height: Option<u64>,
    pub start_offset: Option<u64>,
    pub end_offset: Option<u64>,
    pub turbo: bool,
}

#[derive(Clone, Debug)]
pub struct Output {
    pub rune_id: (u64, u32),
    pub tx_hash: [u8; 32],
    pub etching: EtchingInfo,
    pub cenotaph: bool,
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
                Some(Dunestone {
                    etching, cenotaph, ..
                }) => match etching {
                    Some(etch) => {
                        let name = name_map.get(&(idx as u32)).cloned();

                        let info = if cenotaph {
                            EtchingInfo {
                                name,
                                spacers: Some(0),
                                symbol: None,
                                divisibility: Some(0),
                                max_mint_txs: None,
                                amount_per_mint: None,
                                start_height: None,
                                end_height: None,
                                start_offset: None,
                                end_offset: None,
                                turbo: false,
                            }
                        } else {
                            EtchingInfo {
                                name,
                                spacers: etch.spacers,
                                symbol: etch.symbol.map(|x| x.into()),
                                divisibility: etch.divisibility,
                                max_mint_txs: etch.terms.map(|t| t.cap).flatten(),
                                amount_per_mint: etch.terms.map(|t| t.limit).flatten(),
                                start_height: etch.terms.map(|x| x.height.0).flatten(),
                                end_height: etch.terms.map(|x| x.height.1).flatten(),
                                start_offset: etch.terms.map(|x| x.offset.0).flatten(),
                                end_offset: etch.terms.map(|x| x.offset.1).flatten(),
                                turbo: etch.turbo,
                            }
                        };

                        let etch = ReducerOutput::EtchingByRuneId(Output {
                            rune_id: (height, idx as u32),
                            tx_hash: tx.compute_txid().to_byte_array(),
                            etching: info,
                            cenotaph,
                        });

                        outputs.push(etch)
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

        super::Reducer::EtchingByRuneId(reducer)
    }
}
