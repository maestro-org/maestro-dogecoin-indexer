use axum::{extract::Query, response::IntoResponse, Extension, Json};
use ordinals::{Rune, SpacedRune};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::etching_by_rune_id::{Key as EtchingKey, Value as EtchingValue},
    Encode, Reducer,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{dunes::DuneIdAndName, CountParam, CursorPaginationParams, PaginatedResponse},
    util::ParsedPaginationParams,
};

#[utoipa::path(
    tag = "Dunes",
    get,
    path = "/assets/dunes",
    params(
        ("count" = inline(Option<CountParam>), Query, description = "The max number of results per page"),
        ("cursor" = inline(Option<String>), Query, description = "Pagination cursor string, use the cursor included in a page of results to fetch the next page"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = PaginatedDuneIdAndName,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "LIST_DUNES", level = "info", skip(polyphony))]
/// List Dunes
///
/// Returns a list of ID and names of all deployed Dune assets.
pub async fn list_dunes(
    page_params: Query<CursorPaginationParams>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let encoder = polyphony.etching_by_rune_id_encoder()?;

    // --- start db snapshot at most recent timestamp

    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // --- fetch cursor key for last updated

    let last_updated = snapshot.get_last_updated(&encoder).await?;

    // --- initialise `next_cursor`

    let mut next_cursor = None;

    // --- scan total balances by address

    let page_params = ParsedPaginationParams::parse_no_height::<_, EtchingKey>(
        page_params.0,
        &encoder,
        &Reducer::EtchingByRuneId,
        None::<u64>,
    )?;

    let kvs = Scanner::new(page_params.key_range())
        .count(page_params.count() + 1)
        .execute::<EtchingKey, EtchingValue>(&mut snapshot)
        .await?;

    // --- process fetched kvs

    let mut kvs = kvs.into_iter().enumerate();

    let mut tickers: Vec<DuneIdAndName> = Vec::new();

    while let Some((i, (key, info))) = kvs.next() {
        // if this is the last result of the page, check if there is a subsequent
        // result (and therefore we need to return a cursor for next page)
        if i == (page_params.count() - 1) && kvs.next().is_some() {
            next_cursor = Some(key.encode_base64());
        };

        let rune = Rune(
            info.name
                .ok_or_else(|| Error::Internal("expected rune name in dogecoin".into()))?,
        );

        let spaced_name = SpacedRune {
            rune,
            spacers: info.spacers.unwrap_or(0),
        }
        .to_string();

        tickers.push(DuneIdAndName {
            id: format!("{}:{}", key.rune_id.0, key.rune_id.1),
            spaced_name,
        });
    }

    let out = PaginatedResponse {
        data: tickers,
        last_updated,
        next_cursor,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": [{
        "id": "5228806:44",
        "spaced_name": "JUST•SEND•ITTTT"
    }, {
        "id": "5228811:6",
        "spaced_name": "MOMMAWEMADEIT"
    }, {
        "id": "5228827:57",
        "spaced_name": "BOOK•ON•DUNESSS"
    }, {
        "id": "5228850:19",
        "spaced_name": "TODAY•TOMORROW"
    }, {
        "id": "5228851:15",
        "spaced_name": "TOMORROW•TODAY"
    }, {
        "id": "5229366:35",
        "spaced_name": "PEPE•MEME•KING"
    }, {
        "id": "5244144:12",
        "spaced_name": "DOGECOIN•FIWB"
    }],
    "last_updated": {
        "block_hash": "85bd0d5594a7cd024ee6b3f345ea2e8ca0725a96fe751f5487202064dbb64b1c",
        "block_height": 5251959
    },
    "next_cursor": null
}"##;
