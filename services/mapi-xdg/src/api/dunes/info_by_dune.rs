use axum::{extract::Path, response::IntoResponse, Extension, Json};
use bitcoin::{hashes::Hash, Txid};
use ordinals::{Rune, SpacedRune};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::{
        balances_by_rune_id::Key as BalancesByRuneIdKey,
        etching_by_rune_id::{Key, Value},
        mints_by_rune_id, reducer_key_range,
    },
    Reducer, VarUInt,
};

use crate::{
    client::ChainClient,
    error::Error,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{
        dunes::{DuneInfo, Terms},
        TimestampedResponse,
    },
    util::{decimal, RuneIdentifier},
};

#[utoipa::path(
    tag = "Dunes",
    get,
    path = "/assets/dunes/{dune}",
    params(
        ("dune" = String, Path, description = "Dune, specified either by the Dune ID (etching block number and transaction index) or name (spaced or un-spaced)", example="2519999:31"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = TimestampedDuneInfo,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "INFO_BY_DUNE", level = "info", skip(polyphony))]
/// Dunes Info
///
/// Returns information about the specified Dune, including etching information,
/// current supply and number of holders.
pub async fn info_by_dune(
    Path(rune_id): Path<String>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
) -> Result<impl IntoResponse, Error> {
    let etching_encoder = &polyphony.etching_by_rune_id_encoder()?;
    let mints_encoder = &polyphony.mints_by_rune_id_encoder()?;
    let balances_encoder = &polyphony.balances_by_rune_id_encoder()?;

    // --- start db snapshot at most recent timestamp

    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // -- parse and try decode user params

    let rune_id = match RuneIdentifier::parse(rune_id)? {
        RuneIdentifier::Id(x) => x,
        RuneIdentifier::Name(n) => polyphony
            .resolve_rune_name(&mut snapshot, n)
            .await?
            .ok_or_else(|| Error::NotFound)?,
    };

    // --- fetch cursor for last updated

    let last_updated = snapshot.get_last_updated(etching_encoder).await?;

    // --- fetch data

    let info = snapshot
        .get_reducer_key_maybe::<_, Value>(
            etching_encoder,
            &Reducer::EtchingByRuneId,
            &Key { rune_id },
        )
        .await?
        .ok_or_else(|| Error::NotFound)?;

    let total_mints = snapshot
        .get_reducer_key_maybe::<_, u128>(
            mints_encoder,
            &Reducer::MintsByRuneId,
            &mints_by_rune_id::Key { rune_id },
        )
        .await?;

    // --- fetch rune holders

    let (balances_range_lower, balances_range_upper) = reducer_key_range(
        balances_encoder.namespace(),
        &Reducer::BalancesByRuneId,
        &Some((VarUInt::from(rune_id.0), VarUInt::from(rune_id.1))),
        None::<u64>,
        None::<u64>,
    );

    let kvs = Scanner::new(balances_range_lower..balances_range_upper)
        .execute::<BalancesByRuneIdKey, u128>(&mut snapshot)
        .await?;

    let total_holders = kvs.len();

    let circulating_supply: u128 = kvs.into_iter().map(|(_, x)| x).sum();

    // ---

    // max supply = premine + (max mint txs + amount per mint)
    let max_supply = if let Some(max) = info.max_mint_txs {
        Some(
            info.premine
                .unwrap_or(0)
                .saturating_add(max.saturating_mul(info.amount_per_mint.unwrap_or(0))),
        )
    } else if info.amount_per_mint == Some(0) {
        Some(info.premine.unwrap_or(0))
    } else {
        None
    };

    let rune = Rune(
        info.name
            .ok_or_else(|| Error::Internal("expected rune name in dogecoin".into()))?,
    );

    let spaced_rune = SpacedRune {
        rune,
        spacers: info.spacers.unwrap_or(0),
    };

    let dec = info.divisibility.unwrap_or_default() as usize;

    let out = DuneInfo {
        id: format!("{}:{}", rune_id.0, rune_id.1),
        etching_cenotaph: info.cenotaph,
        etching_tx: Txid::from_byte_array(info.tx_hash).to_string(),
        etching_height: rune_id.0,
        name: rune.to_string(),
        spaced_name: spaced_rune.to_string(),
        symbol: info.symbol.map(|x| char::from_u32(x).unwrap_or(' ')),
        divisibility: info.divisibility.unwrap_or_default(),
        terms: Terms {
            mint_txs_cap: info.max_mint_txs.map(|x| x.to_string()),
            amount_per_mint: info.amount_per_mint.map(|x| decimal(x, dec)),
            start_height: info.start_height.map(|x| x.to_string()),
            end_height: info.end_height.map(|x| x.to_string()),
            start_offset: info.start_offset.map(|x| x.to_string()),
            end_offset: info.end_offset.map(|x| x.to_string()),
        },
        max_supply: max_supply.map(|x| decimal(x, dec)),
        circulating_supply: decimal(circulating_supply, info.divisibility.unwrap_or(0) as usize),
        mints: total_mints.unwrap_or(0) as u64,
        unique_holders: total_holders as u64,
    };

    let out = TimestampedResponse {
        data: out,
        last_updated,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": {
        "id": "5280961:140",
        "etching_cenotaph": false,
        "etching_tx": "39a7cc8155a9e68692ee280e2dfb5a4007cfa5f83f2801abf10fb2682ee3fabc",
        "etching_height": 5280961,
        "name": "TESTTESTTEEST",
        "spaced_name": "TEST•TEST•TEEST",
        "symbol": "a",
        "divisibility": 8,
        "terms": {
            "mint_txs_cap": "1000000000",
            "amount_per_mint": "1000.00000000",
            "start_height": "5280850",
            "end_height": "5290000",
            "start_offset": null,
            "end_offset": null
        },
        "max_supply": "1000000000000.00000000",
        "mints": 2,
        "unique_holders": 2
    },
    "last_updated": {
        "block_hash": "304f07cefebe68472f2ceb02706f9a3b5a9e9ecb3534ec34923799c8ecd7f27d",
        "block_height": 5303197
    }
}"##;
