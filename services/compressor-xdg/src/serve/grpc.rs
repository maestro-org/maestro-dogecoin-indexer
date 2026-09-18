use std::{collections::HashMap, pin::Pin};

use bitcoin::{hashes::Hash, BlockHash};
use brc20_action::Kind;
use futures_core::Stream;
use futures_util::StreamExt;
use tonic::{Request, Response, Status};
use tracing::{error, info};

use crate::storage::{
    chain::BlockByHeightKV,
    inscriptions::updater::BRC20Message,
    kvtable::{DBInt, DBSerde, KVTable},
    mutable::{Log, MutableKV},
    resolver::ResolverByHeightKV,
    ChainDB, NewInscriptionInfo, TxoBody, TxoRef,
};

pub mod compressor_api {
    tonic::include_proto!("compressorxbt.sync.v1"); // The string specified here must match the proto package name
}

use compressor_api::*;

use self::stream_updates_with_ctx_response::Action;

pub struct SyncServerImpl {
    chain_db: ChainDB,
}

impl SyncServerImpl {
    pub fn new(chain_db: ChainDB) -> Self {
        Self { chain_db }
    }
}

#[async_trait::async_trait]
impl sync_service_server::SyncService for SyncServerImpl {
    type StreamUpdatesWithContextStream =
        Pin<Box<dyn Stream<Item = Result<StreamUpdatesWithCtxResponse, Status>> + Send + 'static>>;

    async fn page_blocks_with_context(
        &self,
        request: Request<PageBlocksWithCtxRequest>,
    ) -> Result<Response<PageBlocksWithCtxResponse>, Status> {
        let request = request.into_inner();

        if request.max_items < 1 {
            return Err(Status::invalid_argument("max items must be greater than 0"));
        }

        let max_items = request.max_items as usize;

        info!(
            "received page blocks with context request, {max_items} items from cursor: {:?}",
            request.cursor.as_ref().map(|x| hex::encode(&x.hash))
        );

        let db_tx = self.chain_db.db.snapshot();

        // --- start chain iterator and check intersect on chain

        let intersect_hash = request.cursor.clone().map(|x| x.hash);
        let intersect_height = request
            .cursor
            .as_ref()
            .map(|x| x.height)
            .unwrap_or_default();

        let instant_a = tokio::time::Instant::now();

        let mut block_by_height_iter = BlockByHeightKV::iter_entries_from_snapshot(
            &self.chain_db.db,
            &db_tx,
            DBInt(intersect_height),
        );

        // if we have an intersect hash, get the intersect entry from block by height
        if let Some(hash) = intersect_hash.clone() {
            let (DBInt(found_height), DBSerde((found_hash, _))) = block_by_height_iter
                .next()
                .ok_or(Status::not_found("intersect not found (no entry)"))?
                .map_err(Status::internal)?;

            if found_height != intersect_height {
                return Err(Status::not_found("intersect not found (height mismatch)"));
            };

            if found_hash.to_byte_array().to_vec() != hash {
                return Err(Status::not_found("intersect not found (hash mismatch)"));
            }
        }

        // --- take page of blocks

        let mut page_blocks = block_by_height_iter
            .take(max_items + 1)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Status::internal)?;

        let next_cursor = if page_blocks.len() == max_items + 1 {
            page_blocks.remove(max_items);
            page_blocks
                .last()
                .map(|(DBInt(height), DBSerde((hash, _)))| BlockRef {
                    height: *height,
                    hash: hash.to_byte_array().to_vec(),
                })
        } else {
            None
        };

        let blockfetch_duration = instant_a.elapsed();

        // --- fetch resolvers

        let instant_b = tokio::time::Instant::now();

        let mut resolver_by_height_iter = ResolverByHeightKV::iter_entries_from_snapshot(
            &self.chain_db.db,
            &db_tx,
            DBInt(intersect_height),
        );

        if intersect_hash.is_some() {
            resolver_by_height_iter.next(); // skip intersect if needed
        }

        let page_resolvers = resolver_by_height_iter
            .take(max_items)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Status::internal)?;

        let resolving_duration = instant_b.elapsed();

        // ---

        if page_blocks.len() != page_resolvers.len() {
            error!(
                "blocks/resolvers len mismatch {:?} vs {:?}",
                page_blocks
                    .iter()
                    .map(|x| x.0 .0.clone())
                    .collect::<Vec<_>>(),
                page_resolvers
                    .iter()
                    .map(|x| x.0 .0.clone())
                    .collect::<Vec<_>>()
            );
            return Err(Status::internal("blocks/resolvers len mismatch"));
        }

        let mut page = vec![];

        for (block, block_resolver) in page_blocks.into_iter().zip(page_resolvers) {
            let (DBInt(height), DBSerde((block_hash, bytes))) = block;
            let (
                _,
                DBSerde((
                    resolver_hash,
                    resolver,
                    output_runes_resolver,
                    etchs,
                    mints,
                    output_inscriptions_resolver,
                    valid_reinscriptions,
                    _,
                    brc20_actions,
                    new_inscriptions,
                )),
            ) = block_resolver;

            if block_hash != resolver_hash {
                error!(
                    "blocks/resolvers hash mismatch ({}, {}, {})",
                    height, block_hash, resolver_hash
                );

                return Err(Status::internal("blocks/resolvers hash mismatch"));
            }

            let runes_info = {
                let mut runes_resolver: Vec<RunesResolvedTxo> = resolver
                    .clone()
                    .into_iter()
                    .map(
                        |(TxoRef(ref_hash, ref_index), TxoBody { dunes, .. })| RunesResolvedTxo {
                            r#ref: Some(compressor_api::TxoRef {
                                tx_hash: ref_hash.as_byte_array().to_vec(),
                                txo_index: ref_index as u32,
                            }),
                            runes: dunes
                                .into_iter()
                                .map(|(r, amount)| compressor_api::Rune {
                                    id: Some(compressor_api::RuneId {
                                        block: r.block,
                                        tx: r.tx as u32,
                                    }),
                                    amount: u128::to_be_bytes(amount).to_vec(),
                                })
                                .collect(),
                        },
                    )
                    .collect();

                let output_runes: Vec<RunesResolvedTxo> = output_runes_resolver
                    .into_iter()
                    .map(|(TxoRef(ref_hash, ref_index), runes)| RunesResolvedTxo {
                        r#ref: Some(compressor_api::TxoRef {
                            tx_hash: ref_hash.as_byte_array().to_vec(),
                            txo_index: ref_index as u32,
                        }),
                        runes: runes
                            .into_iter()
                            .map(|(r, amount)| compressor_api::Rune {
                                id: Some(compressor_api::RuneId {
                                    block: r.block,
                                    tx: r.tx as u32,
                                }),
                                amount: u128::to_be_bytes(amount).to_vec(),
                            })
                            .collect(),
                    })
                    .collect();

                runes_resolver.extend(output_runes);

                let etchs = etchs
                    .into_iter()
                    .map(|(tx, name)| compressor_api::ValidEtch {
                        tx_index: tx,
                        name: name.to_be_bytes().to_vec(),
                    })
                    .collect();

                let mints = mints
                    .into_iter()
                    .map(|(tx, edict)| compressor_api::ValidMint {
                        tx_index: tx,
                        edict_index: edict.into(),
                    })
                    .collect();

                Some(RunesInfo {
                    txo_resolver: runes_resolver,
                    successful_etchs: etchs,
                    successful_mints: mints,
                })
            };

            let inscriptions_info = {
                let mut inscriptions_resolver: HashMap<TxoRef, Vec<InscriptionAtOffset>> =
                    HashMap::new();

                for (txoref, body) in resolver.clone() {
                    for (offset, inscription) in body.inscriptions {
                        let x = InscriptionAtOffset {
                            id: Some(InscriptionId {
                                tx_hash: inscription.txid.to_byte_array().to_vec(),
                                index: inscription.index,
                            }),
                            offset: offset as u32,
                        };

                        inscriptions_resolver
                            .entry(txoref.clone())
                            .or_default()
                            .push(x);
                    }
                }

                let mut inscriptions_resolver: Vec<InscriptionResolvedTxo> = inscriptions_resolver
                    .into_iter()
                    .map(
                        |(TxoRef(ref_hash, ref_index), inscriptions)| InscriptionResolvedTxo {
                            r#ref: Some(compressor_api::TxoRef {
                                tx_hash: ref_hash.as_byte_array().to_vec(),
                                txo_index: ref_index as u32,
                            }),
                            inscriptions,
                        },
                    )
                    .collect();

                // outputs

                let mut inscriptions_resolver_outs: HashMap<TxoRef, Vec<InscriptionAtOffset>> =
                    HashMap::new();

                for (txoref, ins) in output_inscriptions_resolver {
                    for (offset, inscription) in ins {
                        let x = InscriptionAtOffset {
                            id: Some(InscriptionId {
                                tx_hash: inscription.txid.to_byte_array().to_vec(),
                                index: inscription.index,
                            }),
                            offset: offset as u32,
                        };

                        inscriptions_resolver_outs
                            .entry(txoref.clone())
                            .or_default()
                            .push(x);
                    }
                }

                let output_inscs: Vec<InscriptionResolvedTxo> = inscriptions_resolver_outs
                    .into_iter()
                    .map(
                        |(TxoRef(ref_hash, ref_index), inscriptions)| InscriptionResolvedTxo {
                            r#ref: Some(compressor_api::TxoRef {
                                tx_hash: ref_hash.as_byte_array().to_vec(),
                                txo_index: ref_index as u32,
                            }),
                            inscriptions,
                        },
                    )
                    .collect();

                inscriptions_resolver.extend(output_inscs);

                let valid_reinscriptions = valid_reinscriptions
                    .into_iter()
                    .map(|(x, y)| ValidReinscription {
                        tx_index: x,
                        inscription_index: y,
                    })
                    .collect();

                let brc20_resolver = brc20_actions
                    .into_iter()
                    .map(|(id, action)| {
                        let x = match action {
                            BRC20Message::Deploy(ticker) => Kind::Deploy(Brc20Deploy { ticker }),
                            BRC20Message::Mint(ticker, amt, sh) => Kind::Mint(Brc20Mint {
                                ticker,
                                amt: amt.to_be_bytes().to_vec(),
                                script_hash: sh.as_byte_array().to_vec(),
                            }),
                            BRC20Message::Transfer(ticker, amt, op, sh) => {
                                let txo_ref = compressor_api::TxoRef {
                                    tx_hash: op.txid.as_byte_array().to_vec(),
                                    txo_index: op.vout,
                                };

                                Kind::Transfer(Brc20Transfer {
                                    ticker,
                                    amt: amt.to_be_bytes().to_vec(),
                                    first_output: Some(txo_ref),
                                    script_hash: sh.as_byte_array().to_vec(),
                                })
                            }
                            BRC20Message::TransferInit(ticker, amt, sh) => {
                                Kind::TransferInit(Brc20TransferInit {
                                    ticker,
                                    amt: amt.to_be_bytes().to_vec(),
                                    script_hash: sh.as_byte_array().to_vec(),
                                })
                            }
                        };

                        Brc20Resolver {
                            id: Some(InscriptionId {
                                tx_hash: id.txid.as_byte_array().to_vec(),
                                index: id.index,
                            }),
                            action: Some(Brc20Action { kind: Some(x) }),
                        }
                    })
                    .collect();
                let new_inscriptions: Vec<InscriptionOrigin> = new_inscriptions
                    .into_iter()
                    .map(|inscription: NewInscriptionInfo| {
                        let tx_hash = inscription.tx_hash.to_vec();
                        let id: InscriptionId = InscriptionId {
                            tx_hash: inscription.id.txid.as_byte_array().to_vec(),
                            index: inscription.id.index,
                        };
                        InscriptionOrigin {
                            tx_hash,
                            id: Some(id),
                            num: inscription.number,
                            content: inscription.body,
                            r#type: inscription.body_type,
                        }
                    })
                    .collect();

                Some(InscriptionsInfo {
                    txo_resolver: inscriptions_resolver,
                    valid_reinscriptions,
                    brc20_resolver,
                    new_inscriptions,
                })
            };

            let txo_resolver = resolver
                .into_iter()
                .map(
                    |(
                        TxoRef(ref_hash, ref_index),
                        TxoBody {
                            height: txo_height,
                            raw,
                            ord_ranges: ords,
                            ..
                        },
                    )| ResolvedTxo {
                        r#ref: Some(compressor_api::TxoRef {
                            tx_hash: ref_hash.as_byte_array().to_vec(),
                            txo_index: ref_index as u32,
                        }),
                        height: txo_height,
                        raw,
                        ord_ranges: ords
                            .to_vec()
                            .into_iter()
                            .map(|r| compressor_api::OrdinalRange {
                                lower: r.lower(),
                                upper: r.upper(),
                            })
                            .collect(),
                    },
                )
                .collect();

            page.push(BlockWithContext {
                r#ref: Some(BlockRef {
                    height,
                    hash: block_hash.as_byte_array().to_vec(),
                }),
                raw: bytes,
                txo_resolver,
                runes: runes_info,
                inscriptions: inscriptions_info,
            });
        }

        // ---

        let chain_tip = match BlockByHeightKV::iter_entries_snapshot(
            &self.chain_db.db,
            &db_tx,
            rocksdb::IteratorMode::End,
        )
        .next()
        {
            Some(entry) => {
                let (DBInt(tip_height), DBSerde((tip_hash, _))) =
                    entry.map_err(Status::internal)?;

                Some(BlockRef {
                    height: tip_height,
                    hash: tip_hash.to_byte_array().to_vec(),
                })
            }
            None => None,
        };

        // ---

        let response = PageBlocksWithCtxResponse {
            blocks: page,
            next_cursor,
            chain_tip,
        };

        info!(
            "finished processing page with context req (fetching: {:?}ms, resolving: {:?}ms)",
            blockfetch_duration.as_millis(),
            resolving_duration.as_millis()
        );

        Ok(Response::new(response))
    }

    async fn stream_updates_with_context(
        &self,
        request: Request<StreamUpdatesWithCtxRequest>,
    ) -> std::result::Result<Response<Self::StreamUpdatesWithContextStream>, Status> {
        let db_tx = self.chain_db.db.snapshot();

        let request = request.into_inner();

        for intersect in request.intersects {
            let BlockRef {
                height: i_height,
                hash: i_hash,
            } = intersect;

            let i_hash: [u8; 32] = i_hash
                .try_into()
                .map_err(|_| Status::invalid_argument("invalid intersect block hash"))?;

            let maybe_wal_seq = MutableKV::find_wal_seq(
                &self.chain_db.db,
                &db_tx,
                i_height,
                BlockHash::from_byte_array(i_hash),
            )
            .map_err(Status::internal)?;

            if let Some(wal_seq) = maybe_wal_seq {
                let stream = MutableKV::stream_mutable(&self.chain_db, wal_seq).map(|x| match x {
                    Ok(log) => Ok(log_to_response_with_ctx(log)),
                    Err(_) => Err(Status::internal("streamupdateswithctx returned error")),
                });

                return Ok(Response::new(Box::pin(stream)));
            }
        }

        return Err(Status::not_found(
            "no intersect found with mutable part of chain",
        ));
    }
}

fn log_to_response_with_ctx(log: Log) -> StreamUpdatesWithCtxResponse {
    let action = match log {
        Log::Apply(
            height,
            hash,
            body,
            resolver,
            output_runes_resolver,
            etchs,
            mints,
            output_inscriptions_resolver,
            valid_reinscriptions,
            _,
            brc20_actions,
            new_inscriptions,
        ) => {
            let runes_info = {
                let mut runes_resolver: Vec<RunesResolvedTxo> = resolver
                    .clone()
                    .into_iter()
                    .map(
                        |(TxoRef(ref_hash, ref_index), TxoBody { dunes, .. })| RunesResolvedTxo {
                            r#ref: Some(compressor_api::TxoRef {
                                tx_hash: ref_hash.as_byte_array().to_vec(),
                                txo_index: ref_index as u32,
                            }),
                            runes: dunes
                                .into_iter()
                                .map(|(r, amount)| compressor_api::Rune {
                                    id: Some(compressor_api::RuneId {
                                        block: r.block,
                                        tx: r.tx as u32,
                                    }),
                                    amount: u128::to_be_bytes(amount).to_vec(),
                                })
                                .collect(),
                        },
                    )
                    .collect();

                let output_runes: Vec<RunesResolvedTxo> = output_runes_resolver
                    .into_iter()
                    .map(|(TxoRef(ref_hash, ref_index), runes)| RunesResolvedTxo {
                        r#ref: Some(compressor_api::TxoRef {
                            tx_hash: ref_hash.as_byte_array().to_vec(),
                            txo_index: ref_index as u32,
                        }),
                        runes: runes
                            .into_iter()
                            .map(|(r, amount)| compressor_api::Rune {
                                id: Some(compressor_api::RuneId {
                                    block: r.block,
                                    tx: r.tx as u32,
                                }),
                                amount: u128::to_be_bytes(amount).to_vec(),
                            })
                            .collect(),
                    })
                    .collect();

                runes_resolver.extend(output_runes);

                let etchs = etchs
                    .into_iter()
                    .map(|(tx, name)| compressor_api::ValidEtch {
                        tx_index: tx,
                        name: name.to_be_bytes().to_vec(),
                    })
                    .collect();

                let mints = mints
                    .into_iter()
                    .map(|(tx, edict)| compressor_api::ValidMint {
                        tx_index: tx,
                        edict_index: edict.into(),
                    })
                    .collect();

                Some(RunesInfo {
                    txo_resolver: runes_resolver,
                    successful_etchs: etchs,
                    successful_mints: mints,
                })
            };

            let inscriptions_info = {
                let mut inscriptions_resolver: HashMap<TxoRef, Vec<InscriptionAtOffset>> =
                    HashMap::new();

                for (txoref, body) in resolver.clone() {
                    for (offset, inscription) in body.inscriptions {
                        let x = InscriptionAtOffset {
                            id: Some(InscriptionId {
                                tx_hash: inscription.txid.to_byte_array().to_vec(),
                                index: inscription.index,
                            }),
                            offset: offset as u32,
                        };

                        inscriptions_resolver
                            .entry(txoref.clone())
                            .or_default()
                            .push(x);
                    }
                }

                let mut inscriptions_resolver: Vec<InscriptionResolvedTxo> = inscriptions_resolver
                    .into_iter()
                    .map(
                        |(TxoRef(ref_hash, ref_index), inscriptions)| InscriptionResolvedTxo {
                            r#ref: Some(compressor_api::TxoRef {
                                tx_hash: ref_hash.as_byte_array().to_vec(),
                                txo_index: ref_index as u32,
                            }),
                            inscriptions,
                        },
                    )
                    .collect();

                // outputs

                let mut inscriptions_resolver_outs: HashMap<TxoRef, Vec<InscriptionAtOffset>> =
                    HashMap::new();

                for (txoref, ins) in output_inscriptions_resolver {
                    for (offset, inscription) in ins {
                        let x = InscriptionAtOffset {
                            id: Some(InscriptionId {
                                tx_hash: inscription.txid.to_byte_array().to_vec(),
                                index: inscription.index,
                            }),
                            offset: offset as u32,
                        };

                        inscriptions_resolver_outs
                            .entry(txoref.clone())
                            .or_default()
                            .push(x);
                    }
                }

                let output_inscs: Vec<InscriptionResolvedTxo> = inscriptions_resolver_outs
                    .into_iter()
                    .map(
                        |(TxoRef(ref_hash, ref_index), inscriptions)| InscriptionResolvedTxo {
                            r#ref: Some(compressor_api::TxoRef {
                                tx_hash: ref_hash.as_byte_array().to_vec(),
                                txo_index: ref_index as u32,
                            }),
                            inscriptions,
                        },
                    )
                    .collect();

                inscriptions_resolver.extend(output_inscs);

                let valid_reinscriptions = valid_reinscriptions
                    .into_iter()
                    .map(|(x, y)| ValidReinscription {
                        tx_index: x,
                        inscription_index: y,
                    })
                    .collect();

                let brc20_resolver = brc20_actions
                    .into_iter()
                    .map(|(id, action)| {
                        let x = match action {
                            BRC20Message::Deploy(ticker) => Kind::Deploy(Brc20Deploy { ticker }),
                            BRC20Message::Mint(ticker, amt, sh) => Kind::Mint(Brc20Mint {
                                ticker,
                                amt: amt.to_be_bytes().to_vec(),
                                script_hash: sh.as_byte_array().to_vec(),
                            }),
                            BRC20Message::Transfer(ticker, amt, op, sh) => {
                                let txo_ref = compressor_api::TxoRef {
                                    tx_hash: op.txid.as_byte_array().to_vec(),
                                    txo_index: op.vout,
                                };

                                Kind::Transfer(Brc20Transfer {
                                    ticker,
                                    amt: amt.to_be_bytes().to_vec(),
                                    first_output: Some(txo_ref),
                                    script_hash: sh.as_byte_array().to_vec(),
                                })
                            }
                            BRC20Message::TransferInit(ticker, amt, sh) => {
                                Kind::TransferInit(Brc20TransferInit {
                                    ticker,
                                    amt: amt.to_be_bytes().to_vec(),
                                    script_hash: sh.as_byte_array().to_vec(),
                                })
                            }
                        };

                        Brc20Resolver {
                            id: Some(InscriptionId {
                                tx_hash: id.txid.as_byte_array().to_vec(),
                                index: id.index,
                            }),
                            action: Some(Brc20Action { kind: Some(x) }),
                        }
                    })
                    .collect();
                let new_inscriptions: Vec<InscriptionOrigin> = new_inscriptions
                    .into_iter()
                    .map(|inscription: NewInscriptionInfo| {
                        let tx_hash = inscription.tx_hash.to_vec();
                        let id: InscriptionId = InscriptionId {
                            tx_hash: inscription.id.txid.as_byte_array().to_vec(),
                            index: inscription.id.index,
                        };
                        InscriptionOrigin {
                            tx_hash,
                            id: Some(id),
                            num: inscription.number,
                            content: inscription.body,
                            r#type: inscription.body_type,
                        }
                    })
                    .collect();

                Some(InscriptionsInfo {
                    txo_resolver: inscriptions_resolver,
                    valid_reinscriptions,
                    brc20_resolver,
                    new_inscriptions,
                })
            };

            let txo_resolver = resolver
                .into_iter()
                .map(
                    |(
                        TxoRef(ref_hash, ref_index),
                        TxoBody {
                            height: txo_height,
                            raw,
                            ord_ranges: ords,
                            ..
                        },
                    )| ResolvedTxo {
                        r#ref: Some(compressor_api::TxoRef {
                            tx_hash: ref_hash.to_byte_array().to_vec(),
                            txo_index: ref_index as u32,
                        }),
                        height: txo_height,
                        raw,
                        ord_ranges: ords
                            .to_vec()
                            .into_iter()
                            .map(|r| compressor_api::OrdinalRange {
                                lower: r.lower(),
                                upper: r.upper(),
                            })
                            .collect(),
                    },
                )
                .collect();

            let block_with_ctx = BlockWithContext {
                r#ref: Some(BlockRef {
                    height,
                    hash: hash.to_byte_array().to_vec(),
                }),
                raw: body,
                txo_resolver,
                runes: runes_info,
                inscriptions: inscriptions_info,
            };

            Action::Apply(block_with_ctx)
        }
        Log::Undo(height, hash, _) => Action::Undo(BlockRef {
            height,
            hash: hash.to_byte_array().to_vec(),
        }),
        Log::Mark(height, hash, _) => Action::Reset(BlockRef {
            height,
            hash: hash.to_byte_array().to_vec(),
        }),
    };

    StreamUpdatesWithCtxResponse {
        action: Some(action),
    }
}
