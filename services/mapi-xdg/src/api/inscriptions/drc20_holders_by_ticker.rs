use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    Extension, Json,
};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::{balances_by_brc20::Key as Brc20BalanceKey, brc20_terms_by_ticker},
    Encode, Reducer, ShortByteString,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{inscriptions::Drc20Holder, CountParam, CursorPaginationParams, PaginatedResponse},
    util::{decimal, ParsedPaginationParams},
};

#[utoipa::path(
    tag = "DRC20",
    get,
    path = "/assets/drc20/{ticker}/holders",
    params(
        ("ticker" = String, Path, description = "DRC20 ticker string", example="TUAH"),

        ("count" = inline(Option<CountParam>), Query, description = "The max number of results per page"),
        ("cursor" = inline(Option<String>), Query, description = "Pagination cursor string, use the cursor included in a page of results to fetch the next page"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = PaginatedDrc20Holder,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "DRC20_HOLDERS", level = "info", skip(polyphony))]
/// DRC20 Holders
///
/// Script pubkeys or addresses that hold the specified DRC20
/// token and corresponding total balances.
pub async fn drc20_holders_by_ticker(
    page_params: Query<CursorPaginationParams>,
    Path(ticker): Path<String>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let balances_encoder = &polyphony.brc20_balances_by_ticker_encoder()?;
    let terms_encoder = &polyphony.brc20_terms_by_ticker_encoder()?;
    let script_by_script_hash_encoder = &polyphony.script_by_script_hash_encoder()?;

    // --- start db snapshot at most recent timestamp

    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // -- parse and try decode user params

    let ticker = ticker.clone();

    let ticker = ticker.to_lowercase();

    // --- fetch cursor key for last updated

    let last_updated = snapshot.get_last_updated(balances_encoder).await?;

    // --- initialise `next_cursor`

    let mut next_cursor = None;

    // --- scan total balances by ticker

    let page_params = ParsedPaginationParams::parse_no_height::<_, [u8; 20]>(
        page_params.0,
        balances_encoder,
        &Reducer::BalancesByBrc20,
        Some(ShortByteString(ticker.as_bytes().to_vec())),
    )?;

    let kvs = Scanner::new(page_params.key_range())
        .count(page_params.count() + 1)
        .execute::<Brc20BalanceKey, u128>(&mut snapshot)
        .await?;

    // --- process fetched kvs

    let mut kvs = kvs.into_iter().enumerate();

    let mut holders: Vec<Drc20Holder> = Vec::new();

    while let Some((i, (key, value))) = kvs.next() {
        // if this is the last result of the page, check if there is a subsequent
        // result (and therefore we need to return a cursor for next page)
        if i == (page_params.count() - 1) && kvs.next().is_some() {
            next_cursor = Some(key.script_hash.encode_base64());
        };

        let dec = snapshot
            .get_reducer_key::<_, brc20_terms_by_ticker::Value>(
                terms_encoder,
                &Reducer::Brc20TermsByTicker,
                &brc20_terms_by_ticker::Key {
                    ticker: key.ticker.clone(),
                },
            )
            .await?
            .dec as usize;

        let (address, script) = polyphony
            .resolve_script_hash(
                &mut snapshot,
                mode.0,
                key.script_hash,
                script_by_script_hash_encoder,
            )
            .await?;

        holders.push(Drc20Holder {
            address: address.map(|x| x.to_string()),
            script_pubkey: hex::encode(script),
            balance: decimal(value, dec),
        })
    }

    let out = PaginatedResponse {
        data: holders,
        last_updated,
        next_cursor,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": [{
        "address": "DNG3G7pKc1DgciyWs36GFUnVyvJieDhKdb",
        "script_pubkey": "76a9142cbe0e5f8f21b1b1a5d1d0d9e3a2b4c5d6e7f80988ac",
        "balance": "9000000.000000000000000000"
    }],
    "last_updated": {
        "block_hash": "1c3e7cd9e46bd6d0adb1ae0b52ec1a1ddfaa0cb61a41a1b1c26f0d1b0f42a06c",
        "block_height": 5150534
    },
    "next_cursor": "19FwuaejD9hE1R4ckTQKaqe0ecA"
}"##;
