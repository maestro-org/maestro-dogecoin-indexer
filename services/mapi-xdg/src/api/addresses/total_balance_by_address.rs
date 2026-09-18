use crate::{
    client::ChainClient, polyphony::PolyphonyWrapper, types::TimestampedResponse,
    util::parse_address_or_script_bytes,
};
use axum::{extract::Path, response::IntoResponse, Extension, Json};
use bitcoin::{hashes::Hash, Script};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::sat_balance_by_script_hash::{
        Key as SatBalanceByScriptHashKey, Value as SatBalanceByScriptHashValue,
    },
    Reducer,
};

use crate::{error::Error, options::Mode};

#[utoipa::path(
    tag = "Addresses",
    get,
    path = "/addresses/{address}/balance",
    params(
        ("address" = String, Path, description = "Dogecoin address or hex encoded script pubkey", example="DNG3G7pKc1DgciyWs36GFUnVyvJieDhKdb"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = TimestampedTotalBalanceByAddress,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "TOTAL_BALANCE_BY_ADDRESS", level = "info", skip(polyphony))]
/// Total Balance by Address
///
/// Sum of koinu in UTxOs which reside at the specified address or script pubkey.
pub async fn total_balance_by_address(
    Path(addr_or_pk): Path<String>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    // SatBalanceByScriptHash encoder.
    let total_balance_encoder = polyphony.sat_balance_by_script_hash_encoder()?;

    // Snapshot and last updated.
    let mut snapshot = polyphony.begin_snapshot_latest().await?;
    let last_updated = snapshot.get_last_updated(&total_balance_encoder).await?;

    // Parse address and fetch script from database.
    let (_, script_bytes) =
        parse_address_or_script_bytes(&polyphony, &mut snapshot, addr_or_pk, mode.0).await?;
    let script = Script::from_bytes(&script_bytes);
    let script_hash = script.script_hash();

    // Fetch total balance data for queried address.
    let satoshis = snapshot
        .get_reducer_key_maybe::<SatBalanceByScriptHashKey, SatBalanceByScriptHashValue>(
            &total_balance_encoder,
            &Reducer::SatBalanceByScriptHash,
            &SatBalanceByScriptHashKey {
                script_hash: script_hash.to_byte_array(),
            },
        )
        .await?
        .map(|value| value.satoshis)
        .unwrap_or(0);

    // Build response.
    let out = TimestampedResponse {
        data: satoshis.to_string(),
        last_updated,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": "695100",
    "last_updated": {
        "block_hash": "1c3e7cd9e46bd6d0adb1ae0b52ec1a1ddfaa0cb61a41a1b1c26f0d1b0f42a06c",
        "block_height": 5150991
    }
}"##;
