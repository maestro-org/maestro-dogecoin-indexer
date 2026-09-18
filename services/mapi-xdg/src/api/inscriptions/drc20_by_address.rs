use std::collections::HashMap;

use axum::{extract::Path, response::IntoResponse, Extension, Json};
use bitcoin::{hashes::Hash, Script};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::{
        brc20_balances_by_script_hash::Key as Brc20BalanceKey, brc20_terms_by_ticker,
        reducer_key_range,
    },
    Reducer,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{inscriptions::Drc20Balances, TimestampedResponse},
    util::{decimal, parse_address_or_script_bytes},
};

#[utoipa::path(
    tag = "Addresses",
    get,
    path = "/addresses/{address}/drc20",
    params(
        ("address" = String, Path, description = "Dogecoin address or hex encoded script pubkey", example="DNG3G7pKc1DgciyWs36GFUnVyvJieDhKdb"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = TimestampedDrc20Quantities,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "DRC20_BY_ADDRESS", level = "info", skip(polyphony))]
/// DRC20 by Address
///
/// Map of all DRC20 tokens and corresponding total and available balances
/// controlled by the specified address or script pubkey.
pub async fn drc20_by_address(
    Path(addr_or_pk): Path<String>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let balances_encoder = &polyphony.brc20_balances_by_script_hash_encoder()?;
    let terms_encoder = &polyphony.brc20_terms_by_ticker_encoder()?;

    // --- start db snapshot at most recent timestamp

    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // -- parse and try decode user params

    let (_, script_bytes) =
        parse_address_or_script_bytes(&polyphony, &mut snapshot, addr_or_pk, mode.0).await?;

    let script = Script::from_bytes(&script_bytes);
    let script_hash = script.script_hash();

    // --- fetch cursor key  for last updated

    let last_updated = snapshot.get_last_updated(balances_encoder).await?;

    // --- scan total balances by address

    let (total_range_lower, total_range_upper) = reducer_key_range(
        balances_encoder.namespace(),
        &Reducer::Brc20TotalBalanceByScriptHash,
        &Some(script_hash.to_byte_array()),
        None::<u64>,
        None::<u64>,
    );

    let total_kvs = Scanner::new(total_range_lower..total_range_upper)
        .execute::<Brc20BalanceKey, u128>(&mut snapshot)
        .await?;

    let (available_range_lower, available_range_upper) = reducer_key_range(
        balances_encoder.namespace(),
        &Reducer::Brc20AvailableBalanceByScriptHash,
        &Some(script_hash.to_byte_array()),
        None::<u64>,
        None::<u64>,
    );

    let available_kvs = Scanner::new(available_range_lower..available_range_upper)
        .execute::<Brc20BalanceKey, u128>(&mut snapshot)
        .await?;

    if total_kvs.len() != available_kvs.len() {
        return Err(Error::Internal("drc20 balance len mismatch".into()));
    }

    // --- process fetched kvs

    let mut brc20_balances = HashMap::new();

    for ((total_k, total_v), (available_k, available_v)) in total_kvs.into_iter().zip(available_kvs)
    {
        if total_k.ticker.0 != available_k.ticker.0 {
            return Err(Error::Internal("drc20 balance ticker mismatch".into()));
        }

        // omit tokens with both 0 amounts
        if total_v == 0 && available_v == 0 {
            continue;
        };

        let dec = snapshot
            .get_reducer_key::<_, brc20_terms_by_ticker::Value>(
                terms_encoder,
                &Reducer::Brc20TermsByTicker,
                &brc20_terms_by_ticker::Key {
                    ticker: total_k.ticker.clone(),
                },
            )
            .await?
            .dec as usize;

        brc20_balances.insert(
            String::from_utf8_lossy(&total_k.ticker.0).to_string(),
            Drc20Balances {
                total: decimal(total_v, dec),
                available: decimal(available_v, dec),
            },
        );
    }

    let out = TimestampedResponse {
        data: brc20_balances,
        last_updated,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": {
        "ABCD": {
            "total": "312000.000",
            "available": "0.123"
        }
    },
    "last_updated": {
        "block_hash": "000000009ed3f5385c1807ca04630b9b2273398670726f93282fd41ba88dc6b8",
        "block_height": 4413542
    }
}"##;
