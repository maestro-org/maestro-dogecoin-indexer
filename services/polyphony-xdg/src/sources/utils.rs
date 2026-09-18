use bitcoin::{hashes::Hash, BlockHash};
use tonic::{transport::Channel, Code};

use crate::{
    crosscut::{self, Point},
    sources::compressor::compressor_api::PageBlocksWithCtxRequest,
    storage, Error,
};

use super::compressor::compressor_api::{sync_service_client::SyncServiceClient, BlockRef};

// Must match the compressor's chain_db.immutable_after_confs
pub static IMMUTABLE_AFTER_BLOCKS: u64 = 1000;

pub fn point_to_block_ref(point: Point) -> BlockRef {
    BlockRef {
        height: point.height,
        hash: point.hash.to_byte_array().to_vec(),
    }
}

pub fn block_ref_to_point(block_ref: BlockRef) -> Point {
    Point {
        hash: BlockHash::from_slice(&block_ref.hash).expect("block ref"),
        height: block_ref.height,
    }
}

pub async fn try_intersect_with_last_point_or_config(
    intersect: &crosscut::IntersectConfig,
    cursor: &mut storage::Cursor,
    client: &mut SyncServiceClient<Channel>,
) -> Result<Option<Point>, crate::Error> {
    match cursor.last_point().await? {
        Some(point) => {
            tracing::info!("found existing cursor in storage plugin: {:?}", point);

            let res = client
                .page_blocks_with_context(PageBlocksWithCtxRequest {
                    cursor: Some(point_to_block_ref(point.clone())),
                    max_items: 1,
                })
                .await;

            match res {
                Ok(_) => {
                    tracing::info!("found intersect with cursor: {:?}", point);
                    return Ok(Some(point));
                }
                Err(e) if e.code() == Code::NotFound => {
                    tracing::error!("could not intersect using cursor: {:?}", point);
                    return Err(Error::IntersectNotFound);
                }
                Err(e) => return Err(Error::source(e)),
            }
        }
        None => tracing::info!("no cursor found in storage plugin"),
    };

    match &intersect {
        crosscut::IntersectConfig::Origin => {
            tracing::info!("using origin as intersect");

            Ok(None)
        }
        crosscut::IntersectConfig::Tip => {
            let res = client
                .page_blocks_with_context(PageBlocksWithCtxRequest {
                    cursor: None,
                    max_items: 1,
                })
                .await
                .map_err(Error::source)?;

            let tip = res.into_inner().chain_tip;

            tracing::info!("using source tip as intersect: {:?}", tip);

            Ok(tip.map(|x| block_ref_to_point(x)))
        }
        crosscut::IntersectConfig::Point(_, _) => {
            let point = intersect.get_point().expect("point value");

            let res = client
                .page_blocks_with_context(PageBlocksWithCtxRequest {
                    cursor: Some(point_to_block_ref(point.clone())),
                    max_items: 1,
                })
                .await;

            match res {
                Ok(_) => {
                    tracing::info!("found intersect with config point: {:?}", point);
                    return Ok(Some(point));
                }
                Err(e) if e.code() == Code::NotFound => {
                    tracing::error!("could not intersect using config point: {:?}", point);
                    return Err(Error::IntersectNotFound);
                }
                Err(e) => return Err(Error::source(e)),
            }
        }
    }
}

pub async fn try_intersect_with_rollback_buf(
    cursor: &mut storage::Cursor,
    client: &mut SyncServiceClient<Channel>,
) -> Result<Option<Point>, crate::Error> {
    match cursor.fetch_persistent_buffer().await? {
        Some(buf) => {
            // sort our persistent buf points newest to oldest
            let points: Vec<Point> = buf.into_iter().map(|v| v.point.into()).rev().collect();

            tracing::info!(
                "found persistent rollback buffer in storage plugin with points: {:?}",
                points
            );

            // for each point, keep trying to request blocks from that point
            // until we get a success, which means we intersected with the chain
            // at that point
            for point in points {
                let res = client
                    .page_blocks_with_context(PageBlocksWithCtxRequest {
                        cursor: Some(point_to_block_ref(point.clone())),
                        max_items: 1,
                    })
                    .await;

                match res {
                    Ok(_) => {
                        tracing::info!("found intersect with rb buffer: {:?}", point);
                        return Ok(Some(point));
                    }
                    Err(e) if e.code() == Code::NotFound => (),
                    Err(e) => return Err(Error::source(e)),
                }
            }

            Err(Error::RollbackOutOfRange(
                "no intersect found with persistent rb buffer (maybe upstream catching up?)".into(),
            ))
        }
        None => {
            tracing::info!("no persistent rollback buffer found");

            Ok(None)
        }
    }
}
