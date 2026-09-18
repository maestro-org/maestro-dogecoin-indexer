use std::collections::HashMap;

use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    Extension, Json,
};
use bitcoin::{hashes::Hash, Script};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::{etching_by_rune_id, reducer_key_range, rune_balances_by_script_hash::Key},
    Reducer,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{HeightPaginationParams, TimestampedResponse},
    util::{decimal, parse_address_or_script_bytes},
};

#[utoipa::path(
    tag = "Addresses",
    get,
    path = "/addresses/{address}/dunes",
    params(
        ("address" = String, Path, description = "Dogecoin address or hex encoded script pubkey", example="DNG3G7pKc1DgciyWs36GFUnVyvJieDhKdb"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = TimestampedDuneQuantities,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "DUNES_BY_ADDRESS", level = "info", skip(polyphony))]
/// Dunes by Address
///
/// Returns a map of all Dunes and corresponding amounts in UTxOs controlled by
/// the specified address or script pubkey.
pub async fn dunes_by_address(
    page_params: Query<HeightPaginationParams>,
    Path(addr_or_pk): Path<String>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let balances_encoder = &polyphony.rune_balances_by_script_hash_encoder()?;
    let etching_encoder = &polyphony.etching_by_rune_id_encoder()?;

    // --- start db snapshot at most recent timestamp

    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // -- parse and try decode user params

    let (_, script_bytes) =
        parse_address_or_script_bytes(&polyphony, &mut snapshot, addr_or_pk, mode.0).await?;

    let script = Script::from_bytes(&script_bytes);
    let script_hash = script.script_hash();

    // --- fetch cursor key  for last updated

    let last_updated = snapshot.get_last_updated(balances_encoder).await?;

    // --- scan balances for address

    let (balances_range_lower, balances_range_upper) = reducer_key_range(
        balances_encoder.namespace(),
        &Reducer::RuneBalancesByScriptHash,
        &Some(script_hash.to_byte_array()),
        None::<u64>,
        None::<u64>,
    );

    let kvs = Scanner::new(balances_range_lower..balances_range_upper)
        .execute::<Key, u128>(&mut snapshot)
        .await?;

    // --- process fetched kvs

    let mut rune_balances = HashMap::new();

    for (k, amount) in kvs {
        let (rid_block, rid_tx) = k.rune_id;

        let dec = snapshot
            .get_reducer_key::<_, etching_by_rune_id::Value>(
                etching_encoder,
                &Reducer::EtchingByRuneId,
                &etching_by_rune_id::Key { rune_id: k.rune_id },
            )
            .await?
            .divisibility
            .unwrap_or(0) as usize;

        rune_balances.insert(format!("{}:{}", rid_block, rid_tx), decimal(amount, dec));
    }

    let out = TimestampedResponse {
        data: rune_balances,
        last_updated,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": {
        "5244142:13": "2100000.00000000",
        "5244114:7": "0.01000000",
        "5244141:12": "2100000.00000000",
        "5244142:14": "2100000.00000000",
        "5244144:12": "2100000.00000000"
    },
    "last_updated": {
        "block_hash": "85bd0d5594a7cd024ee6b3f345ea2e8ca0725a96fe751f5487202064dbb64b1c",
        "block_height": 5251959
    }
}"##;
