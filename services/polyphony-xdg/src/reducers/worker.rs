use bitcoin::{consensus::Decodable, Block, BlockHash};
use gasket::runtime::{ScheduleResult, WorkSchedule};
use tracing::{debug, error};

use crate::{
    crosscut::Point,
    model::{self, EnrichedBlockPayload, StorageActionPayload},
    prelude::*,
};

use super::Reducer;

type InputPort = gasket::messaging::tokio::InputPort<model::EnrichedBlockPayload>;
type OutputPort = gasket::messaging::tokio::OutputPort<model::StorageActionPayload>;

pub struct Worker {
    input: InputPort,
    output: OutputPort,
    reducers: Vec<Reducer>,
    ops_count: gasket::metrics::Counter,
    last_block: gasket::metrics::Gauge,
    last_processed: Option<BlockHash>,
}

impl Worker {
    pub fn new(reducers: Vec<Reducer>, input: InputPort, output: OutputPort) -> Self {
        Worker {
            reducers,
            input,
            output,
            ops_count: Default::default(),
            last_block: Default::default(),
            last_processed: None,
        }
    }

    async fn reduce_block(
        &mut self,
        point: Point,
        block: &Vec<u8>,
        ctx: &model::BlockContext,
        mutable: bool,
    ) -> Result<(), gasket::error::Error> {
        debug!("reducing block {:?}", point);

        let block = Block::consensus_decode(&mut &block[..])
            .map_err(crate::Error::encoding)
            .or_panic()?;

        if let Some(prev) = self.last_processed {
            if prev != block.header.prev_blockhash {
                error!(
                    "previous block hash mismatch: {} vs {}",
                    prev, block.header.prev_blockhash
                );
            }
        }

        let is_mempool = false;

        self.last_block.set(point.height as i64);

        let mut outputs = Vec::new();

        // Instead of passing the output port to the reducers, we pass a vec which
        // we will add all the storage actions to, then we will send these down
        // the outport port later.
        for reducer in self.reducers.iter_mut() {
            reducer.reduce_block(point.height, &block, ctx, &mut outputs)?;
            self.ops_count.inc(1);
        }

        let current_ts = chrono::Utc::now().timestamp();

        outputs.push(super::ReducerOutput::Cursor(
            point.clone(),
            is_mempool,
            current_ts as u64,
        ));

        debug!(
            "finished reducing block {:?} resulting in {} outputs",
            point,
            outputs.len()
        );

        self.output
            .send(gasket::messaging::Message::from(
                StorageActionPayload::RollForward(point, outputs, mutable, false),
            ))
            .await?;

        self.last_processed = Some(point.hash);

        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl gasket::runtime::Worker for Worker {
    type WorkUnit = EnrichedBlockPayload;

    fn metrics(&self) -> gasket::metrics::Registry {
        gasket::metrics::Builder::new()
            .with_counter("ops_count", &self.ops_count)
            .with_gauge("last_block", &self.last_block)
            .build()
    }

    async fn schedule(&mut self) -> ScheduleResult<Self::WorkUnit> {
        let msg = self.input.recv().await?;

        Ok(WorkSchedule::Unit(msg.payload))
    }

    async fn execute(&mut self, unit: &Self::WorkUnit) -> Result<(), gasket::error::Error> {
        match unit {
            model::EnrichedBlockPayload::RollForward(point, block, ctx, mutable) => {
                self.reduce_block(point.clone(), block, &ctx.clone(), mutable.clone())
                    .await
            }
            // notify storage stage of the rollback, which will handle reversing
            // the storage actions
            model::EnrichedBlockPayload::RollBack(point, mutable) => {
                self.last_processed = Some(point.hash.clone());

                self.output
                    .send(gasket::messaging::Message::from(
                        StorageActionPayload::RollBack(point.clone(), *mutable),
                    ))
                    .await
            }
        }
    }
}
