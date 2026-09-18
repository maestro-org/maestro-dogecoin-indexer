use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{dunes::DuneHolder, CountParam, CursorPaginationParams, PaginatedResponse},
    util::{decimal, ParsedPaginationParams, RuneIdentifier},
};
use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    Extension, Json,
};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::{balances_by_rune_id::Key as BalancesByRuneIdKey, etching_by_rune_id},
    Encode, Reducer, VarUInt,
};

#[utoipa::path(
    tag = "Dunes",
    get,
    path = "/assets/dunes/{dune}/holders",
    params(
        ("dune" = String, Path, description = "Dune, specified either by the Dune ID (etching block number and transaction index) or name (spaced or un-spaced)", example="5095478:8"),
        ("count" = inline(Option<CountParam>), Query, description = "The max number of results per page"),
        ("cursor" = inline(Option<String>), Query, description = "Pagination cursor string, use the cursor included in a page of results to fetch the next page"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = PaginatedDuneHolder,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "HOLDERS_BY_DUNE", level = "info", skip(polyphony))]
/// Holders by Dune
///
/// List of all addresses that hold the specified Dune, with the respective amounts.
pub async fn holders_by_dune(
    page_params: Query<CursorPaginationParams>,
    Path(dune_id): Path<String>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let balances_encoder = &polyphony.balances_by_rune_id_encoder()?;
    let etching_encoder = &polyphony.etching_by_rune_id_encoder()?;
    let script_by_script_hash_encoder = &polyphony.script_by_script_hash_encoder()?;

    // --- start db snapshot at most recent timestamp

    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // --- fetch cursor key for last updated

    let last_updated = snapshot.get_last_updated(balances_encoder).await?;

    // --- parse Rune ID

    let rune_id = match RuneIdentifier::parse(dune_id)? {
        RuneIdentifier::Id(id) => Some(id),
        RuneIdentifier::Name(n) => polyphony.resolve_rune_name(&mut snapshot, n).await?,
    };

    // an unknown dune has no holders
    let Some(rune_id) = rune_id else {
        let out: PaginatedResponse<DuneHolder> = PaginatedResponse {
            data: vec![],
            last_updated,
            next_cursor: None,
        };
        return Ok((StatusCode::OK, Json(out)));
    };

    // --- initialise `next_cursor`

    let mut next_cursor = None;

    // --- scan total balances by ticker

    let page_params = ParsedPaginationParams::parse_no_height::<_, [u8; 20]>(
        page_params.0,
        &balances_encoder,
        &Reducer::BalancesByRuneId,
        Some((VarUInt::from(rune_id.0), VarUInt::from(rune_id.1))),
    )?;

    let kvs = Scanner::new(page_params.key_range())
        .count(page_params.count() + 1)
        .execute::<BalancesByRuneIdKey, u128>(&mut snapshot)
        .await?;

    // --- process fetched kvs

    let mut kvs = kvs.into_iter().enumerate();

    let mut holders: Vec<DuneHolder> = Vec::new();

    let dec = snapshot
        .get_reducer_key_maybe::<_, etching_by_rune_id::Value>(
            etching_encoder,
            &Reducer::EtchingByRuneId,
            &etching_by_rune_id::Key { rune_id },
        )
        .await?
        .and_then(|v| v.divisibility)
        .unwrap_or(0);

    while let Some((i, (key, value))) = kvs.next() {
        // if this is the last result of the page, check if there is a subsequent
        // result (and therefore we need to return a cursor for next page)
        if i == (page_params.count() - 1) && kvs.next().is_some() {
            next_cursor = Some(key.script_hash.encode_base64());
        };

        let (address, script) = polyphony
            .resolve_script_hash(
                &mut snapshot,
                mode.0,
                key.script_hash,
                script_by_script_hash_encoder,
            )
            .await?;

        holders.push(DuneHolder {
            address: address.map(|x| x.to_string()),
            script_pubkey: hex::encode(script),
            balance: decimal(value, dec as usize),
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
        "address": "D8UC42uehZdBYAfMBJumQKuBNs9vvarxtt",
        "script_pubkey": "76a9142cbe0e5f8f21b1b1a5d1d0d9e3a2b4c5d6e7f80988ac",
        "balance": "9000000.000000000000000000"
    }, {
        "address": "DEvbE2KyPKAzDsj511wijyF1NSBN9qMcix",
        "script_pubkey": "76a914e52f1c1b6a2f7d3c4b5a69788736251409f3ab8788ac",
        "balance": "420000.000000000000000000"
    }],
    "last_updated": {
        "block_hash": "1c3e7cd9e46bd6d0adb1ae0b52ec1a1ddfaa0cb61a41a1b1c26f0d1b0f42a06c",
        "block_height": 5150534
    },
    "next_cursor": "19FwuaejD9hE1R4ckTQKaqe0ecA"
}"##;
