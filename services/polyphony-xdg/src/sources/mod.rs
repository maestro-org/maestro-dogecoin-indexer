use gasket::messaging::tokio::OutputPort;
use serde::Deserialize;

use crate::{bootstrap, crosscut, model, storage};

pub mod compressor;

pub mod utils;

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum Config {
    Compressor(compressor::Config),
}

impl Config {
    pub fn bootstrapper(self, intersect: &crosscut::IntersectConfig) -> Bootstrapper {
        match self {
            Config::Compressor(c) => Bootstrapper::Compressor(c.bootstrapper(intersect)),
        }
    }
}

pub enum Bootstrapper {
    Compressor(compressor::Bootstrapper),
}

impl Bootstrapper {
    pub fn borrow_output_port(&mut self) -> &'_ mut OutputPort<model::EnrichedBlockPayload> {
        match self {
            Bootstrapper::Compressor(p) => p.borrow_output_port(),
        }
    }

    pub fn spawn_stages(
        self,
        pipeline: &mut bootstrap::Pipeline,
        cursor: storage::Cursor,
        timeout: u64,
    ) {
        match self {
            Bootstrapper::Compressor(p) => p.spawn_stages(pipeline, cursor, timeout),
        }
    }
}
