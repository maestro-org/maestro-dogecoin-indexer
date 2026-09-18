use crate::util::{decimal, parse_address_or_script_bytes};
use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    Extension, Json,
};
use bitcoin::{hashes::Hash, Script, Txid};
use reqwest::StatusCode;
use serde::Deserialize;
use std::str::FromStr;
use timbre_xbt::{
    builder::BREAK,
    reducers::{
        brc20_terms_by_ticker,
        transfer_inscriptions_by_script_hash::{
            Cursor as TransferInscriptionsByScriptHashCursor,
            Key as TransferInscriptionsByScriptHashKey,
            Value as TransferInscriptionsByScriptHashValue,
        },
    },
    Encode, Reducer, ShortByteString,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{
        inscriptions::TransferInscriptionByAddress, CountParam, CursorPaginationParams, OrderParam,
        PaginatedResponse,
    },
    util::ParsedPaginationParams,
};

#[derive(Debug, Deserialize)]
pub struct Ticker {
    pub ticker: Option<String>,
}

#[utoipa::path(
    tag = "Addresses",
    get,
    path = "/addresses/{address}/transfer_inscriptions",
    params(
        ("address" = String, Path, description = "Dogecoin address or hex encoded script pubkey", example="DNG3G7pKc1DgciyWs36GFUnVyvJieDhKdb"),
        ("ticker" = inline(Option<String>), Query, description = "DRC20 ticker string", example="TUAH"),

        ("count" = inline(Option<CountParam>), Query, description = "The max number of results per page"),
        ("order" = inline(Option<OrderParam>), Query, description = "The order in which the results are sorted"),
        ("cursor" = inline(Option<String>), Query, description = "Pagination cursor string, use the cursor included in a page of results to fetch the next page"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = PaginatedTransferInscriptionByAddress,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(
    name = "TRANSFER_INSCRIPTIONS_BY_ADDRESS",
    level = "info",
    skip(polyphony)
)]
/// DRC20 Transfer Inscriptions by Address
///
/// List of all unused transfer inscriptions which reside at the specified address or script pubkey.
pub async fn transfer_inscriptions_by_address(
    Path(addr_or_pk): Path<String>,
    Query(ticker_param): Query<Ticker>,
    Query(page_params): Query<CursorPaginationParams>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let transfer_inscriptions_encoder =
        &polyphony.transfer_inscriptions_by_script_hash_encoder()?;
    let terms_encoder = &polyphony.brc20_terms_by_ticker_encoder()?;

    // --- start db snapshot at most recent timestamp
    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // --- fetch cursor key  for last updated
    let last_updated = snapshot
        .get_last_updated(transfer_inscriptions_encoder)
        .await?;

    // --- initialise `next_cursor`
    let mut next_cursor = None;

    // --- parse and try to user params
    let script_hash =
        match parse_address_or_script_bytes(&polyphony, &mut snapshot, addr_or_pk.clone(), mode.0)
            .await
        {
            Ok((_, script_bytes)) => Script::from_bytes(&script_bytes).script_hash(),
            Err(Error::NotFound) => {
                // --- user param is a Dogecoin address, but the corresponding script pub key could
                // --- not be found in store because this address has no transactions history
                let out = PaginatedResponse {
                    data: vec![],
                    last_updated,
                    next_cursor,
                };
                return Ok((StatusCode::OK, Json(out)));
            }
            Err(e) => return Err(e),
        };

    let page_params = if let Some(ticker) = ticker_param.ticker.clone() {
        // if a ticker filter was given, then the cursor is an inscription ID
        ParsedPaginationParams::parse_no_height::<_, ([u8; 32], u32)>(
            page_params,
            transfer_inscriptions_encoder,
            &Reducer::TransferInscriptionsByScriptHash,
            Some((
                script_hash.to_byte_array(),
                BREAK,
                ShortByteString(ticker.into()),
            )),
        )?
    } else {
        // if no ticker filter was given, then the cursor is both the ticker and an inscription ID
        ParsedPaginationParams::parse_no_height::<_, TransferInscriptionsByScriptHashCursor>(
            page_params,
            transfer_inscriptions_encoder,
            &Reducer::TransferInscriptionsByScriptHash,
            Some(script_hash.to_byte_array()),
        )?
    };

    // --- scan keys
    // --- the `+ 1` used in the call to `count` is necessary to compute the cursor for next page
    let kvs = Scanner::new(page_params.key_range())
        .count(page_params.count() + 1)
        .order(page_params.order())
        .execute::<TransferInscriptionsByScriptHashKey, TransferInscriptionsByScriptHashValue>(
            &mut snapshot,
        )
        .await?;

    // --- process fetched kvs
    let mut res = Vec::new();

    let mut kvs_iter = kvs.into_iter().enumerate();

    while let Some((i, (key, value))) = kvs_iter.next() {
        // if this is the last result of the page, check if there is a subsequent
        // result (and therefore we need to return a cursor for next page)
        if i == (page_params.count() - 1) && kvs_iter.next().is_some() {
            next_cursor = if ticker_param.ticker.is_some() {
                Some(key.inscription_id.encode_base64())
            } else {
                Some(
                    TransferInscriptionsByScriptHashCursor {
                        ticker: key.ticker.clone(),
                        inscription_id: key.inscription_id,
                    }
                    .encode_base64(),
                )
            }
        }

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

        res.push(TransferInscriptionByAddress {
            ticker: String::from_utf8_lossy(&key.ticker.0).to_string(),
            inscription_id: format!(
                "{}i{}",
                Txid::from_byte_array(key.inscription_id.0),
                key.inscription_id.1
            ),
            token_amount: decimal(value.token_amount, dec),
            sat_amount: value.sat_amount,
            utxo_txid: Txid::from_byte_array(value.utxo_hash).to_string(),
            utxo_vout: value.utxo_index,
            utxo_sat_offset: value.offset,
            utxo_block_height: value.block_height,
            utxo_confirmations: last_updated.block_height.saturating_sub(value.block_height),
        });
    }

    let out = PaginatedResponse {
        data: res,
        last_updated,
        next_cursor,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": [{
        "ticker": "TUAH",
        "inscription_id": "f02da3d6bebab13d5d604be1ed73d9a9c677dadf6ca71bc5fff7d99cdead11b0i0",
        "token_amount": "1234",
        "sat_amount": 5678,
        "utxo_txid": "e2283e7c915ef074806136e0002cbc69f5fdd2e9f70f14b0eab48cdcbe867cc1",
        "utxo_vout": 0,
        "utxo_sat_offset": 0,
        "utxo_block_height": 843010,
        "utxo_confirmations": 23700
    }],
    "last_updated": {
        "block_hash": "1c3e7cd9e46bd6d0adb1ae0b52ec1a1ddfaa0cb61a41a1b1c26f0d1b0f42a06c",
        "block_height": 5146710
    },
    "next_cursor": "BFRVQUhgfQot2JciKRPVj8lXsEKVJhF6CmHJZGQv6TsHfzKMzsEAAAAA"
}"##;
