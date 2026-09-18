pub mod compressor;

use gasket::messaging::tokio::OutputPort;
use serde::Deserialize;
use std::time::Duration;

use crate::{bootstrap, crosscut, model, storage};

pub mod compressor_api {
    tonic::include_proto!("compressorxbt.sync.v1"); // The string specified here must match the proto package name
}

#[derive(Deserialize, Debug)]
pub struct Config {
    url: String,
    max_items_per_page: Option<u32>,
}

impl Config {
    pub fn bootstrapper(self, intersect: &crosscut::IntersectConfig) -> Bootstrapper {
        Bootstrapper {
            config: self,
            intersect: intersect.clone(),
            output: Default::default(),
        }
    }
}

pub struct Bootstrapper {
    config: Config,
    intersect: crosscut::IntersectConfig,
    output: OutputPort<model::EnrichedBlockPayload>,
}

impl Bootstrapper {
    pub fn borrow_output_port(&mut self) -> &'_ mut OutputPort<model::EnrichedBlockPayload> {
        &mut self.output
    }

    pub fn spawn_stages(
        self,
        pipeline: &mut bootstrap::Pipeline,
        cursor: storage::Cursor,
        timeout: u64,
    ) {
        pipeline.register_stage(gasket::runtime::spawn_stage(
            self::compressor::Worker::new(self.config, self.intersect, cursor, self.output),
            gasket::runtime::Policy {
                tick_timeout: Some(Duration::from_secs(timeout)),
                bootstrap_retry: gasket::retries::Policy {
                    max_retries: 40,
                    backoff_factor: 2,
                    backoff_unit: Duration::from_secs(1),
                    max_backoff: Duration::from_secs(60),
                },
                ..Default::default()
            },
            Some("compressor"),
        ));
    }
}
