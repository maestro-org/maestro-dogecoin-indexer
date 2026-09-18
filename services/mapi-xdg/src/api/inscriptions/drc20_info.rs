use axum::{extract::Path, response::IntoResponse, Extension, Json};
use bitcoin::{hashes::Hash, Txid};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::{
        balances_by_brc20::Key as Brc20BalanceKey, brc20_terms_by_ticker, reducer_key_range,
    },
    Reducer, ShortByteString,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{
        inscriptions::{Drc20Info, Drc20Terms},
        TimestampedResponse,
    },
    util::decimal,
};

#[utoipa::path(
    tag = "DRC20",
    get,
    path = "/assets/drc20/{ticker}",
    params(
        ("ticker" = String, Path, description = "DRC20 ticker string", example="FCTB"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = TimestampedDrc20Info,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "DRC20_INFO", level = "info", skip(polyphony))]
/// DRC20 Info
///
/// Information about the specified DRC20 asset.
pub async fn drc20_info(
    Path(ticker): Path<String>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let balances_encoder = &polyphony.brc20_balances_by_ticker_encoder()?;
    let terms_encoder = &polyphony.brc20_terms_by_ticker_encoder()?;

    // --- start db snapshot at most recent timestamp

    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // -- parse and try decode user params

    let ticker = ticker.clone();

    let ticker = ticker.to_lowercase();

    // --- fetch cursor key for last updated

    let last_updated = snapshot.get_last_updated(&balances_encoder).await?;

    // --- fetch terms

    let terms = snapshot
        .get_reducer_key_maybe::<_, brc20_terms_by_ticker::Value>(
            terms_encoder,
            &Reducer::Brc20TermsByTicker,
            &brc20_terms_by_ticker::Key {
                ticker: ShortByteString(ticker.as_bytes().to_vec()),
            },
        )
        .await?
        .ok_or_else(|| Error::NotFound)?;

    let dec = terms.dec as usize;

    // --- scan total balances by ticker

    let (balances_range_lower, balances_range_upper) = reducer_key_range(
        &balances_encoder.namespace(),
        &Reducer::BalancesByBrc20,
        &Some(ShortByteString(ticker.as_bytes().to_vec())),
        None::<u64>,
        None::<u64>,
    );

    let kvs = Scanner::new(balances_range_lower..balances_range_upper)
        .execute::<Brc20BalanceKey, u128>(&mut snapshot)
        .await?;

    // --- process fetched kvs

    let holders = kvs.len();
    let minted_supply: u128 = kvs.into_iter().map(|(_, x)| x).sum();

    let deploy_inscription = format!(
        "{}i{}",
        Txid::from_byte_array(terms.deploy_id.0).to_string(),
        terms.deploy_id.1
    );

    let out = TimestampedResponse {
        data: Drc20Info {
            ticker: ticker.clone(),
            ticker_hex: hex::encode(ticker),
            deploy_inscription,
            holders: holders as u64,
            minted_supply: decimal(minted_supply, dec),
            terms: Drc20Terms {
                max: decimal(terms.max, dec),
                limit: decimal(terms.limit, dec),
                dec: terms.dec,
            },
        },
        last_updated,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": {
        "ticker": "$yod",
        "ticker_hex": "24796f64",
        "deploy_inscription": "f86ac394543faf363d18b9bebfe160f6e57539f69642ce9a3d23f2e4e92c12c5i0",
        "holders": 86,
        "minted_supply": "7969155.000000000000000000",
        "terms": {
            "max": "20420420.000000000000000000",
            "limit": "69.000000000000000000",
            "dec": 18
        }
    },
    "last_updated": {
        "block_hash": "bdb39d4aaf7dc588c3732a27fd57b7cdc0c93bec20c98526e3af45217590500a",
        "block_height": 5288700
    }
}"##;
