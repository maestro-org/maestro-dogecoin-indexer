use crate::{types::InvolvedTransaction, util::parse_address_or_script_bytes};
use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    Extension, Json,
};
use bitcoin::{hashes::Hash, Script, Txid};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::txs_by_script_hash::{
        Cursor as TxsByScriptHashCursor, Key as TxsByScriptHashKey, Value as TxsByScriptHashValue,
    },
    Encode, Reducer,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{CountParam, HeightPaginationParams, OrderParam, PaginatedResponse},
    util::ParsedHeightPaginationParams,
};

#[utoipa::path(
    tag = "Addresses",
    get,
    path = "/addresses/{address}/txs",
    params(
        ("address" = String, Path, description = "Dogecoin address or hex encoded script pubkey", example="DNG3G7pKc1DgciyWs36GFUnVyvJieDhKdb"),

        ("count" = inline(Option<CountParam>), Query, description = "The max number of results per page"),

        ("order" = inline(Option<OrderParam>), Query, description = "The order in which the results are sorted (by height at which transaction was included in a block)"),
        ("from" = inline(Option<u64>), Query, description = "Return only transactions included on or after a specific height"),
        ("to" = inline(Option<u64>), Query, description = "Return only transactions included on or before a specific height"),

        ("cursor" = inline(Option<String>), Query, description = "Pagination cursor string, use the cursor included in a page of results to fetch the next page"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = PaginatedInvolvedTransaction,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "TXS_BY_ADDRESS", level = "info", skip(polyphony))]
/// Transactions by Address
///
/// List of all transactions which consumed or produced a UTxO controlled
/// by the specified address or script pubkey.
pub async fn txs_by_address(
    page_params: Query<HeightPaginationParams>,
    Path(addr_or_pk): Path<String>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let encoder = polyphony.txs_by_script_hash_encoder()?;

    // --- start db snapshot at most recent timestamp

    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // -- parse and try decode user params

    let (_, script_bytes) =
        parse_address_or_script_bytes(&polyphony, &mut snapshot, addr_or_pk, mode.0).await?;

    let script = Script::from_bytes(&script_bytes);
    let script_hash = script.script_hash();

    let page_params = ParsedHeightPaginationParams::parse::<_, TxsByScriptHashCursor>(
        page_params.0,
        &encoder,
        &Reducer::TxsByScriptHash,
        Some(script_hash.to_byte_array()),
    )?;

    // --- fetch cursor key  for last updated

    let last_updated = snapshot.get_last_updated(&encoder).await?;

    // --- initialise `next_cursor`

    let mut next_cursor = None;

    // --- scan keys for this page (max count plus one to see if there is another page)

    let kvs = Scanner::new(page_params.key_range())
        .count(page_params.count() + 1)
        .order(page_params.order())
        .execute::<TxsByScriptHashKey, TxsByScriptHashValue>(&mut snapshot)
        .await?;

    // --- process fetched kvs

    let mut kvs = kvs.into_iter().enumerate();

    let mut txs: Vec<InvolvedTransaction> = Vec::new();

    while let Some((i, (key, value))) = kvs.next() {
        // if this is the last result of the page, check if there is a subsequent
        // result (and therefore we need to return a cursor for next page)
        if i == (page_params.count() - 1) && kvs.next().is_some() {
            next_cursor = Some(
                TxsByScriptHashCursor {
                    height: key.height,
                    blk_index: key.blk_index,
                    tx_hash: key.tx_hash,
                }
                .encode_base64(),
            );
        };

        txs.push(InvolvedTransaction {
            tx_hash: Txid::from_byte_array(key.tx_hash).to_string(),
            height: key.height,
            input: value.input,
            output: value.output,
        })
    }

    let out = PaginatedResponse {
        data: txs,
        last_updated,
        next_cursor,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": [{
        "tx_hash": "1cd3a819876660e98d3d5d9e4d36ddbd1ae6f96e58de0d3977f0ef2ce6e4194a",
        "height": 5277680,
        "input": true,
        "output": true
    }, {
        "tx_hash": "ad7b8037fc7551fd9e644ddd39bc0501bc6aac865284fd79dde8b732af45acd9",
        "height": 5277682,
        "input": true,
        "output": true
    }],
    "last_updated": {
        "block_hash": "8ac9689a7901531013c3cd621eae8b8e75b1994f477d616a4faa2a10afd9be58",
        "block_height": 5277710
    },
    "next_cursor": "AAAAAABQh_JgAAlg2axFrzK36N15_YRShqxqvAEFvDndTWSe_VF1_DeAe60"
}"##;
