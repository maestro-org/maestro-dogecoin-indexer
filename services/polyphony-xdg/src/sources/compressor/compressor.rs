use bitcoin::hashes::Hash;
use bitcoin::{OutPoint, ScriptHash, Txid};
use futures_util::StreamExt;
use gasket::runtime::{ScheduleResult, WorkSchedule};
use ordinals::RuneId;
use tonic::codec::CompressionEncoding;
use tracing::{debug, warn};

use gasket::error::AsWorkError;
use tonic::transport::Channel;
use tonic::{Code, Streaming};
use tracing::info;

use crate::inscriptions::inscription_id::InscriptionId;
use crate::model::{BRC20Message, BlockContext, EnrichedBlockPayload};
use crate::sources::utils::{block_ref_to_point, point_to_block_ref, IMMUTABLE_AFTER_BLOCKS};
use crate::{crosscut, model, sources::utils, storage, Error};

use super::compressor_api::brc20_action::Kind;
use super::compressor_api::stream_updates_with_ctx_response::Action;
use super::compressor_api::sync_service_client::SyncServiceClient;
use super::compressor_api::{
    BlockRef, PageBlocksWithCtxRequest, StreamUpdatesWithCtxRequest, StreamUpdatesWithCtxResponse,
    ValidEtch, ValidMint,
};
use super::Config;

pub type OutputPort = gasket::messaging::tokio::OutputPort<model::EnrichedBlockPayload>;

pub struct SourceWorkUnit {
    actions: Vec<Action>,
    mutable: bool,
}

enum Mode {
    Init,
    Dumping(Option<BlockRef>),
    Streaming,
}

pub struct Worker {
    config: Config,
    intersect: crosscut::IntersectConfig,
    cursor: storage::Cursor,
    client: Option<SyncServiceClient<Channel>>,
    mode: Mode,
    stream: Option<Streaming<StreamUpdatesWithCtxResponse>>,
    output: OutputPort,
    upstream_height: Option<u64>,
    block_count: gasket::metrics::Counter,
    chain_tip: gasket::metrics::Gauge,
}

impl Worker {
    pub fn new(
        config: Config,
        intersect: crosscut::IntersectConfig,
        cursor: storage::Cursor,
        output: OutputPort,
    ) -> Self {
        Self {
            config,
            intersect,
            cursor,
            mode: Mode::Init,
            output,
            client: None,
            stream: None,
            upstream_height: None,
            block_count: Default::default(),
            chain_tip: Default::default(),
        }
    }

    async fn process_action(
        &mut self,
        action: Action,
        mutable: bool,
    ) -> Result<(), gasket::error::Error> {
        match action {
            Action::Apply(block) => {
                let block_ref = block
                    .r#ref
                    .ok_or(Error::source("missing block_ref"))
                    .or_panic()?;

                let mut ctx = BlockContext::new();

                for txo in block.txo_resolver {
                    let txo_ref = txo
                        .r#ref
                        .ok_or(Error::source("missing txoref"))
                        .or_panic()?;

                    let ref_tx_hash: [u8; 32] = txo_ref
                        .tx_hash
                        .try_into()
                        .map_err(|_| Error::source("malformed txoref txhash"))
                        .or_panic()?;

                    let output_ref =
                        OutPoint::new(Txid::from_byte_array(ref_tx_hash), txo_ref.txo_index);

                    ctx.insert_txo(&output_ref, txo.height, txo.raw, txo.ord_ranges)
                }

                if let Some(runes_info) = block.runes {
                    for resolved_txo in runes_info.txo_resolver.into_iter() {
                        let txo_ref = resolved_txo
                            .r#ref
                            .ok_or(Error::source("missing txoref"))
                            .or_panic()?;

                        let ref_tx_hash: [u8; 32] = txo_ref
                            .tx_hash
                            .try_into()
                            .map_err(|_| Error::source("malformed txoref txhash"))
                            .or_panic()?;

                        let output_ref =
                            OutPoint::new(Txid::from_byte_array(ref_tx_hash), txo_ref.txo_index);

                        let runes = resolved_txo
                            .runes
                            .into_iter()
                            .map(|x| {
                                let id = x.id.unwrap();
                                let id = RuneId {
                                    block: id.block,
                                    tx: id.tx,
                                };

                                let amount = u128::from_be_bytes(x.amount.try_into().unwrap());

                                (id, amount)
                            })
                            .collect::<Vec<_>>();

                        if !runes.is_empty() {
                            ctx.insert_runes(&output_ref, runes)
                        }
                    }

                    ctx.rune_etch_idxs = runes_info
                        .successful_etchs
                        .into_iter()
                        .map(|ValidEtch { tx_index, name }| {
                            (tx_index, u128::from_be_bytes(name.try_into().unwrap()))
                        })
                        .collect();

                    ctx.rune_mint_idxs = runes_info
                        .successful_mints
                        .into_iter()
                        .map(
                            |ValidMint {
                                 tx_index,
                                 edict_index,
                             }| (tx_index, edict_index),
                        )
                        .collect();
                }

                if let Some(inscriptions_info) = block.inscriptions {
                    for resolved_txo in inscriptions_info.txo_resolver.into_iter() {
                        let txo_ref = resolved_txo
                            .r#ref
                            .ok_or(Error::source("missing txoref"))
                            .or_panic()?;

                        let ref_tx_hash: [u8; 32] = txo_ref
                            .tx_hash
                            .try_into()
                            .map_err(|_| Error::source("malformed txoref txhash"))
                            .or_panic()?;

                        let output_ref =
                            OutPoint::new(Txid::from_byte_array(ref_tx_hash), txo_ref.txo_index);

                        let inscriptions = resolved_txo
                            .inscriptions
                            .into_iter()
                            .map(|x| {
                                let id = x.id.unwrap();

                                let id = InscriptionId {
                                    txid: Txid::from_byte_array(id.tx_hash.try_into().unwrap()),
                                    index: id.index,
                                };

                                (x.offset, id)
                            })
                            .collect::<Vec<_>>();

                        if !inscriptions.is_empty() {
                            ctx.insert_inscriptions(&output_ref, inscriptions)
                        }
                    }

                    ctx.valid_reinscriptions = inscriptions_info
                        .valid_reinscriptions
                        .into_iter()
                        .map(|x| (x.tx_index, x.inscription_index))
                        .collect();

                    for brc20_action in inscriptions_info.brc20_resolver.into_iter() {
                        let id = brc20_action.id.unwrap();

                        let id = InscriptionId {
                            txid: Txid::from_byte_array(id.tx_hash.try_into().unwrap()),
                            index: id.index,
                        };

                        let action = match brc20_action.action.unwrap().kind.unwrap() {
                            Kind::Deploy(x) => BRC20Message::Deploy(x.ticker),
                            Kind::Mint(x) => BRC20Message::Mint(
                                x.ticker,
                                u128::from_be_bytes(x.amt.try_into().unwrap()),
                                ScriptHash::from_byte_array(x.script_hash.try_into().unwrap()),
                            ),
                            Kind::TransferInit(x) => BRC20Message::TransferInit(
                                x.ticker,
                                u128::from_be_bytes(x.amt.try_into().unwrap()),
                                ScriptHash::from_byte_array(x.script_hash.try_into().unwrap()),
                            ),
                            Kind::Transfer(x) => {
                                let first_output = x.first_output.unwrap();

                                let output = OutPoint {
                                    txid: Txid::from_byte_array(
                                        first_output.tx_hash.try_into().unwrap(),
                                    ),
                                    vout: first_output.txo_index,
                                };

                                BRC20Message::Transfer(
                                    x.ticker,
                                    u128::from_be_bytes(x.amt.try_into().unwrap()),
                                    output,
                                    ScriptHash::from_byte_array(x.script_hash.try_into().unwrap()),
                                )
                            }
                        };

                        ctx.insert_brc20(&id, action);
                    }
                    for inscription_origin in inscriptions_info.new_inscriptions.into_iter() {
                        let tx_hash = inscription_origin.tx_hash.try_into().unwrap();

                        let id = inscription_origin.id.unwrap();
                        let id = InscriptionId {
                            txid: Txid::from_byte_array(id.tx_hash.try_into().unwrap()),
                            index: id.index,
                        };

                        let num = inscription_origin.num;

                        let content_type = inscription_origin.r#type.to_vec();

                        let content = inscription_origin.content.to_vec();

                        ctx.insert_new_inscription(&tx_hash, (id, num, content_type, content));
                    }
                }

                let payload = EnrichedBlockPayload::roll_forward(
                    block_ref_to_point(block_ref.clone()),
                    block.raw,
                    ctx,
                    mutable,
                );

                self.output.send(payload).await.or_panic()?;

                self.chain_tip.set(block_ref.height as i64)
            }
            Action::Reset(reset) => {
                let payload =
                    EnrichedBlockPayload::roll_back(block_ref_to_point(reset.clone()), false);

                self.output.send(payload).await.or_panic()?;

                self.chain_tip.set(reset.height as i64);
            }
            Action::Undo(_) => (),
        }

        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl gasket::runtime::Worker for Worker {
    type WorkUnit = SourceWorkUnit;

    fn metrics(&self) -> gasket::metrics::Registry {
        gasket::metrics::Builder::new()
            .with_counter("received_blocks", &self.block_count)
            .with_gauge("chain_tip", &self.chain_tip)
            .build()
    }

    async fn bootstrap(&mut self) -> Result<(), gasket::error::Error> {
        debug!("bootstrapping, config: {:?}", self.config);

        let mut client = SyncServiceClient::connect(self.config.url.clone())
            .await
            .or_panic()?
            .accept_compressed(CompressionEncoding::Gzip)
            .max_decoding_message_size(usize::MAX)
            .max_encoding_message_size(usize::MAX);

        // first try intersect with persistent rollback buffer if one is found
        let intersect = if let Some(point) =
            utils::try_intersect_with_rollback_buf(&mut self.cursor, &mut client)
                .await
                .or_panic()?
        {
            // send rollback to intersect point, in case it is behind our tip
            let payload = EnrichedBlockPayload::roll_back(point.clone(), true);
            self.output.send(payload).await.or_panic()?;

            Some(point)
        } else {
            // otherwise try intersect with cursor in storage, if one is found,
            // or use config if not
            utils::try_intersect_with_last_point_or_config(
                &self.intersect,
                &mut self.cursor,
                &mut client,
            )
            .await
            .or_panic()?
        };

        self.upstream_height = client
            .page_blocks_with_context(PageBlocksWithCtxRequest {
                cursor: None,
                max_items: 1,
            })
            .await
            .or_panic()?
            .into_inner()
            .chain_tip
            .map(|x| x.height);

        if let Some(i) = intersect {
            self.chain_tip.set(i.height as i64);
            self.mode = Mode::Dumping(Some(point_to_block_ref(i)));
        } else {
            self.mode = Mode::Dumping(None);
        }

        self.client = Some(client);

        Ok(())
    }

    async fn schedule(&mut self) -> ScheduleResult<Self::WorkUnit> {
        debug!("scheduling");

        // if we are dumping and in mutable zone, try switch to streaming by
        // intersecting with last processed block
        if let Some(upstream_height) = self.upstream_height {
            if let Mode::Dumping(Some(cursor)) = &self.mode {
                if cursor.height + IMMUTABLE_AFTER_BLOCKS >= upstream_height {
                    let stream_req = StreamUpdatesWithCtxRequest {
                        intersects: vec![cursor.clone()],
                    };

                    info!("within mutable zone ({} v {}), trying to intersect with mutable with {cursor:?}", cursor.height, upstream_height);

                    match self
                        .client
                        .as_mut()
                        .unwrap()
                        .stream_updates_with_context(stream_req)
                        .await
                    {
                        Ok(stream) => {
                            info!("switching to streaming (intersected using {cursor:?})");

                            self.mode = Mode::Streaming;
                            self.stream = Some(stream.into_inner())
                        }
                        Err(err) if err.code() == Code::NotFound => (),
                        e @ Err(_) => {
                            e.or_restart()?;
                        }
                    }
                } else {
                    debug!(
                        "not yet in mutable zone ({} v {})",
                        cursor.height, upstream_height
                    )
                }
            }
        }

        match &self.mode {
            Mode::Init => unreachable!(),
            // if we are streaming, await next action and schedule it
            Mode::Streaming => {
                let stream = self.stream.as_mut().unwrap();
                let next = stream
                    .next()
                    .await
                    .ok_or(Error::source("source stream ended"))
                    .or_restart()?;

                if let Some(action) = next.or_restart()?.action {
                    return Ok(WorkSchedule::Unit(SourceWorkUnit {
                        actions: vec![action],
                        mutable: true,
                    }));
                };

                warn!("stream entry had no action");

                return Ok(WorkSchedule::Idle);
            }
            // if we are dumping, fetch the next page and schedule all the
            // blocks as Apply actions
            Mode::Dumping(cursor) => {
                let dump_request = PageBlocksWithCtxRequest {
                    cursor: cursor.clone(),
                    max_items: self.config.max_items_per_page.unwrap_or(20),
                };

                debug!("compressor requesting page: {dump_request:?}");

                let result = self
                    .client
                    .as_mut()
                    .unwrap()
                    .page_blocks_with_context(dump_request)
                    .await
                    .or_restart()?
                    .into_inner();

                if let Some(last_block) = result.blocks.last() {
                    self.mode = Mode::Dumping(last_block.r#ref.clone());
                }

                self.upstream_height = result.chain_tip.map(|x| x.height);

                let actions: Vec<Action> = result.blocks.into_iter().map(Action::Apply).collect();

                if !actions.is_empty() {
                    debug!("compressor scheduling {} actions", actions.len());
                    Ok(WorkSchedule::Unit(SourceWorkUnit {
                        actions,
                        mutable: false,
                    }))
                } else {
                    warn!("page contained no blocks");
                    Ok(WorkSchedule::Idle)
                }
            }
        }
    }

    async fn execute(&mut self, unit: &Self::WorkUnit) -> Result<(), gasket::error::Error> {
        let mutable = unit.mutable;

        debug!(
            "compressor processing actions (first: {:?})",
            unit.actions.last()
        );
        for action in unit.actions.clone() {
            self.process_action(action, mutable).await.or_panic()?;
        }
        debug!("compressor finished processing actions");

        Ok(())
    }
}
