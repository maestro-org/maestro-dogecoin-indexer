use std::str::from_utf8;

use redis::{FromRedisValue, ToRedisArgs};
use tikv_client::{Timestamp as TiKVTimestamp, TimestampExt};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct RedisKey {
    pub dataplane: u8,
    pub instance: u16,
}

pub fn timestamp_key(dataplane: u8, instance: u16) -> String {
    format!("tikv-timestamps:{}:{}", dataplane, instance)
}

#[derive(Debug, Clone, PartialEq)]
pub struct RedisEntry {
    pub height: u64,
    pub block_hash: [u8; 32],
    pub was_mempool: bool,
    pub commit_ts: TiKVTimestamp,
    pub network: String,
    pub chain_tip_height: u64,
    pub chain_tip_hash: [u8; 32],
    pub mempool_view_ts: u64,
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

            let chain_tip_height = s.next().and_then(|x| x.parse().ok()).unwrap_or(0);

            let chain_tip_hash = s
                .next()
                .and_then(|x| hex::decode(x).ok())
                .and_then(|x| x.try_into().ok())
                .unwrap_or([0; 32]);

            let mempool_view_ts = s.next().and_then(|x| x.parse().ok()).unwrap_or(0);

            redis::RedisResult::Ok(RedisEntry {
                height,
                block_hash,
                was_mempool,
                commit_ts,
                network,
                chain_tip_height,
                chain_tip_hash,
                mempool_view_ts,
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
            self.chain_tip_height.to_string(),
            hex::encode(self.chain_tip_hash),
            self.mempool_view_ts.to_string(),
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
            chain_tip_height: 789,
            chain_tip_hash: [
                2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
                25, 26, 27, 28, 29, 30, 31, 32, 33,
            ],
            mempool_view_ts: 123456789,
        };

        let expected =
            "123,0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20,true,456,mainnet,789,02030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f2021,123456789";
        assert_eq!(entry.to_string(), expected);
    }

    #[test]
    fn test_redis_entry_from_redis_value() {
        let value = Value::SimpleString(
            "123,0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20,true,456,mainnet"
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
        assert_eq!(result.chain_tip_height, 0);
        assert_eq!(result.chain_tip_hash, [0; 32]);
        assert_eq!(result.mempool_view_ts, 0);
    }
}
