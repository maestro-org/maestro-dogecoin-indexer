use axum::{extract::Query, response::IntoResponse, Extension, Json};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::brc20_terms_by_ticker::{Key as Brc20TermsKey, Value as Brc20TermsValue},
    Encode, Reducer, ShortByteString,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{CountParam, CursorPaginationParams, PaginatedResponse},
    util::ParsedPaginationParams,
};

#[utoipa::path(
    tag = "DRC20",
    get,
    path = "/assets/drc20",
    params(
        ("count" = inline(Option<CountParam>), Query, description = "The max number of results per page"),
        ("cursor" = inline(Option<String>), Query, description = "Pagination cursor string, use the cursor included in a page of results to fetch the next page"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = PaginatedDrc20Ticker,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "LIST_DRC20", level = "info", skip(polyphony))]
/// List DRC20
///
/// List of tickers of all deployed DRC20 assets.
pub async fn list_drc20s(
    page_params: Query<CursorPaginationParams>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let encoder = polyphony.brc20_terms_by_ticker_encoder()?;

    // --- start db snapshot at most recent timestamp

    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // --- fetch cursor key for last updated

    let last_updated = snapshot.get_last_updated(&encoder).await?;

    // --- initialise `next_cursor`

    let mut next_cursor = None;

    // --- scan total balances by address

    let page_params = ParsedPaginationParams::parse_no_height::<_, ShortByteString>(
        page_params.0,
        &encoder,
        &Reducer::Brc20TermsByTicker,
        None::<u64>,
    )?;

    let kvs = Scanner::new(page_params.key_range())
        .count(page_params.count() + 1)
        .execute::<Brc20TermsKey, Brc20TermsValue>(&mut snapshot)
        .await?;

    // --- process fetched kvs

    let mut kvs = kvs.into_iter().enumerate();

    let mut tickers: Vec<String> = Vec::new();

    while let Some((i, (key, _))) = kvs.next() {
        // if this is the last result of the page, check if there is a subsequent
        // result (and therefore we need to return a cursor for next page)
        if i == (page_params.count() - 1) && kvs.next().is_some() {
            next_cursor = Some(key.ticker.encode_base64());
        };

        tickers.push(String::from_utf8_lossy(&key.ticker.0).to_string())
    }

    let out = PaginatedResponse {
        data: tickers,
        last_updated,
        next_cursor,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": ["aosi", "ap'q", "fctb"],
    "last_updated": {
        "block_hash": "1c3e7cd9e46bd6d0adb1ae0b52ec1a1ddfaa0cb61a41a1b1c26f0d1b0f42a06c",
        "block_height": 5150368
    },
    "next_cursor": "BGZjdGI"
}"##;
