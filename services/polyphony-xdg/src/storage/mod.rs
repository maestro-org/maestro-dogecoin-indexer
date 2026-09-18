pub mod tikv;

use std::str::from_utf8;

use gasket::messaging::tokio::InputPort;
use redis::{FromRedisValue, ToRedisArgs};
use serde::Deserialize;
use tikv_client::{Timestamp as TiKVTimestamp, TimestampExt};
use tracing::info;

use crate::{
    bootstrap,
    crosscut::{self, Point},
    model,
    rollback::PersistentBufferValue,
};

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum Config {
    TiKV(tikv::Config),
}

impl Config {
    pub fn plugin(
        self,
        policy: &crosscut::policies::RuntimePolicy,
        instance_names: Vec<String>,
    ) -> Bootstrapper {
        match self {
            Config::TiKV(c) => Bootstrapper::TiKV(c.bootstrapper(policy, instance_names)),
        }
    }
}

pub enum Bootstrapper {
    TiKV(tikv::Bootstrapper),
}

impl Bootstrapper {
    pub fn borrow_input_port(&mut self) -> &'_ mut InputPort<model::StorageActionPayload> {
        match self {
            Bootstrapper::TiKV(x) => x.borrow_input_port(),
        }
    }

    pub fn build_cursor(&mut self) -> Cursor {
        info!("building cursor");
        match self {
            Bootstrapper::TiKV(x) => Cursor::TiKV(x.build_cursor()),
        }
    }

    pub fn spawn_stages(
        self,
        pipeline: &mut bootstrap::Pipeline,
        intersect: Option<Point>,
        buf: Option<Vec<PersistentBufferValue>>,
        buffer_size: usize,
        timeout: u64,
    ) {
        match self {
            Bootstrapper::TiKV(x) => x.spawn_stages(pipeline, intersect, buf, buffer_size, timeout),
        }
    }
}

pub enum Cursor {
    TiKV(tikv::Cursor),
}

impl Cursor {
    pub async fn last_point(&mut self) -> Result<Option<Point>, crate::Error> {
        match self {
            Cursor::TiKV(x) => x.last_point().await,
        }
    }

    pub async fn fetch_persistent_buffer(
        &mut self,
    ) -> Result<Option<Vec<PersistentBufferValue>>, crate::Error> {
        info!("fetching persistent buffer");
        match self {
            Cursor::TiKV(x) => x.fetch_persistent_buffer().await,
        }
    }
}

#[derive(Debug)]
pub struct RedisEntry {
    height: u64,
    block_hash: [u8; 32],
    was_mempool: bool,
    commit_ts: TiKVTimestamp,
    network: String,
}

impl FromRedisValue for RedisEntry {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        fn malformed(what: &str) -> redis::RedisError {
            redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "malformed tikv timestamp entry",
                what.to_string(),
            ))
        }

        fn parse(s: &str) -> redis::RedisResult<RedisEntry> {
            let mut s = s.split(',');

            let height = s
                .next()
                .and_then(|x| x.parse().ok())
                .ok_or_else(|| malformed("height"))?;

            let block_hash = s
                .next()
                .and_then(|x| hex::decode(x).ok())
                .and_then(|x| x.try_into().ok())
                .ok_or_else(|| malformed("block hash"))?;

            let was_mempool = s
                .next()
                .and_then(|x| x.parse().ok())
                .ok_or_else(|| malformed("was_mempool"))?;

            let commit_ts = s
                .next()
                .and_then(|x| x.parse().ok())
                .map(TiKVTimestamp::from_version)
                .ok_or_else(|| malformed("commit timestamp"))?;

            let network = s.next().unwrap_or_default().to_string().to_lowercase();

            redis::RedisResult::Ok(RedisEntry {
                height,
                block_hash,
                was_mempool,
                commit_ts,
                network,
            })
        }

        match v {
            redis::Value::SimpleString(s) => parse(s),
            redis::Value::BulkString(s) => {
                parse(from_utf8(s).map_err(|_| malformed("utf-8 entry"))?)
            }
            _ => redis::RedisResult::Err(malformed("unexpected redis value type")),
        }
    }
}

impl ToRedisArgs for RedisEntry {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        out.write_arg(self.to_string().as_bytes());
    }
}

impl ToString for RedisEntry {
    fn to_string(&self) -> String {
        vec![
            self.height.to_string(),
            hex::encode(self.block_hash),
            self.was_mempool.to_string(),
            self.commit_ts.version().to_string(),
            self.network.clone().to_lowercase(),
            // The GC safepoint manager expects the 8-field entry format; with
            // no mempool in this stack, the block is its own chain tip.
            self.height.to_string(),
            hex::encode(self.block_hash),
            0.to_string(),
        ]
        .join(",")
    }
}

#[cfg(test)]
mod tests {
    use redis::Value;

    use super::*;

    #[test]
    fn test_redis_entry_to_string() {
        let entry = RedisEntry {
            height: 123,
            block_hash: [
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27, 28, 29, 30, 31, 32,
            ],
            was_mempool: true,
            commit_ts: TiKVTimestamp::from_version(456),
            network: "mainnet".into(),
            // chain_tip_height: 789,
            // chain_tip_hash: [
            //     2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            //     25, 26, 27, 28, 29, 30, 31, 32, 33,
            // ],
            // mempool_view_ts: 123456789,
        };

        let expected =
            "123,0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20,true,456,mainnet,123,0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20,0";
        assert_eq!(entry.to_string(), expected);
    }

    #[test]
    fn test_redis_entry_from_redis_value() {
        let value = Value::SimpleString(
            "123,0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20,true,456,mainnet,123,0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20,0"
                .to_string(),
        );
        let result = RedisEntry::from_redis_value(&value).unwrap();

        assert_eq!(result.height, 123);
        assert_eq!(
            result.block_hash,
            [
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27, 28, 29, 30, 31, 32
            ]
        );
        assert_eq!(result.was_mempool, true);
        assert_eq!(result.commit_ts.version(), 456);
        assert_eq!(result.network, "mainnet".to_string());
        // assert_eq!(result.chain_tip_height, 0);
        // assert_eq!(result.chain_tip_hash, [0; 32]);
        // assert_eq!(result.mempool_view_ts, 0);
    }
}
