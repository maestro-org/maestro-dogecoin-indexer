use crate::util::parse_address_or_script_bytes;
use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    Extension, Json,
};
use bitcoin::{hashes::Hash, Script, Txid};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::{
        inscription_utxos_by_script_hash::{
            Cursor as InscriptionUtxosByScriptHashCursor, Key as InscriptionUtxosByScriptHashKey,
            Value as InscriptionUtxosByScriptHashValue,
        },
        reducer_key_range,
    },
    Decode, Encode, Reducer, VarUInt,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{
        inscriptions::InscriptionByAddress, CountParam, CursorPaginationParams, PaginatedResponse,
    },
    util::{parse_varuint, MAX_PAGE_COUNT},
};

#[utoipa::path(
    tag = "Addresses",
    get,
    path = "/addresses/{address}/inscriptions",
    params(
        ("address" = String, Path, description = "Dogecoin address or hex encoded script pubkey", example="DNG3G7pKc1DgciyWs36GFUnVyvJieDhKdb"),

        ("count" = inline(Option<CountParam>), Query, description = "The max number of results per page"),
        ("cursor" = inline(Option<String>), Query, description = "Pagination cursor string, use the cursor included in a page of results to fetch the next page"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = PaginatedInscriptionByAddress,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "INSCRIPTIONS_BY_ADDRESS", level = "info", skip(polyphony))]
/// Inscriptions by Address
///
/// List of all inscriptions which reside at the specified address or script pubkey.
pub async fn inscriptions_by_address(
    page_params: Query<CursorPaginationParams>,
    Path(addr_or_pk): Path<String>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let inscription_utxos_encoder = &polyphony.inscription_utxos_by_script_hash_encoder()?;

    // --- start db snapshot at most recent timestamp
    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // --- fetch cursor key  for last updated
    let last_updated = snapshot.get_last_updated(inscription_utxos_encoder).await?;

    // --- initialise `next_cursor`
    let mut next_cursor = None;

    // --- parse and try to decode address
    let script_bytes =
        match parse_address_or_script_bytes(&polyphony, &mut snapshot, addr_or_pk.clone(), mode.0)
            .await
        {
            Ok((_, bytes)) => bytes,
            Err(Error::NotFound) => {
                // --- user param is a Dogecoin address, but the corresponding script pub key could
                // --- not be found in store because this address has no transaction history
                let out = PaginatedResponse {
                    data: vec![],
                    last_updated,
                    next_cursor,
                };
                return Ok((StatusCode::OK, Json(out)));
            }
            Err(e) => return Err(e),
        };

    let script = Script::from_bytes(&script_bytes);
    let script_hash = script.script_hash();

    // --- parse count and cursor params
    let count = match page_params.count {
        Some(CountParam(count)) => {
            if count > MAX_PAGE_COUNT || count == 0 {
                return Err(Error::MalformedRequest("Invalid page size".into()));
            }
            count
        }
        None => MAX_PAGE_COUNT,
    };

    let cursor = if let Some(cursor) = &page_params.cursor {
        match InscriptionUtxosByScriptHashCursor::decode_base64(&cursor) {
            Ok((cursor, _)) => (cursor.tx_id, cursor.index),
            Err(_) => {
                return Err(Error::MalformedRequest(
                    "Error while decoding cursor".into(),
                ))
            }
        }
    } else {
        ([0u8; 32], 0u32.into())
    };

    // --- fetch keys
    let (utxos_range_lower, utxos_range_upper) = reducer_key_range(
        &inscription_utxos_encoder.namespace(),
        &Reducer::InscriptionUtxosByScriptHash,
        &Some(script_hash.to_byte_array()),
        None::<InscriptionUtxosByScriptHashKey>,
        None::<InscriptionUtxosByScriptHashKey>,
    );

    let inscription_utxo_kvs = Scanner::new(utxos_range_lower..utxos_range_upper)
        .execute::<InscriptionUtxosByScriptHashKey, InscriptionUtxosByScriptHashValue>(
            &mut snapshot,
        )
        .await?;

    // --- process: filter and sort inscriptions, truncate response length
    let mut inscriptions: Vec<((u64, String, u32), (String, (u32, ([u8; 32], VarUInt))))> = vec![];

    for (utxo_key, utxo_value) in inscription_utxo_kvs.into_iter() {
        for (offset, inscription_id) in <Vec<_>>::from(utxo_value.inscriptions).into_iter() {
            if cursor < inscription_id {
                inscriptions.push((
                    (
                        parse_varuint(utxo_key.height.clone(), "Invalid block height")?,
                        Txid::from_byte_array(utxo_key.utxo_hash).to_string(),
                        parse_varuint(utxo_key.utxo_index.clone(), "Invalid UTxO index")?,
                    ),
                    (
                        parse_varuint::<u64>(
                            utxo_value.satoshis.clone(),
                            "Invalid satoshis amount in UTxO",
                        )?
                        .to_string(),
                        (
                            parse_varuint(offset, "Invalid inscription offset in UTxO")?,
                            inscription_id,
                        ),
                    ),
                ));
            }
        }
    }

    inscriptions.sort_by_key(|(_, (_, (_, inscription_id)))| inscription_id.clone());

    // --- if response is truncated, update `next_cursor`
    if count < inscriptions.len() {
        let (_, (_, (_, (tx_id, index)))) = inscriptions[count - 1].clone();
        next_cursor = Some(InscriptionUtxosByScriptHashCursor { tx_id, index }.encode_base64());
        inscriptions.truncate(count);
    }

    // --- build response data
    let mut res = Vec::new();

    for ((height, utxo_hash, utxo_index), (satoshis, (offset, inscription_id))) in
        inscriptions.into_iter()
    {
        let inscription_id = format!(
            "{}i{}",
            Txid::from_byte_array(inscription_id.0),
            parse_varuint::<u32>(inscription_id.1, "Invalid inscription index")?,
        );

        let utxo_confirmations = (last_updated.block_height + 1).saturating_sub(height);

        res.push(InscriptionByAddress {
            inscription_id,
            satoshis,
            utxo_sat_offset: offset,
            utxo_txid: utxo_hash,
            utxo_vout: utxo_index,
            utxo_block_height: height,
            utxo_confirmations,
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
        "inscription_id": "f02da3d6bebab13d5d604be1ed73d9a9c677dadf6ca71bc5fff7d99cdead11b0i0",
        "satoshis": "546",
        "utxo_sat_offset": 0,
        "utxo_txid": "e2283e7c915ef074806136e0002cbc69f5fdd2e9f70f14b0eab48cdcbe867cc1",
        "utxo_vout": 0,
        "utxo_block_height": 843010,
        "utxo_confirmations": 23701
    }, {
        "inscription_id": "7d0a2dd897222913d58fc957b0429526117a0a61c964642fe93b077f328ccec1i0",
        "satoshis": "546",
        "utxo_sat_offset": 0,
        "utxo_txid": "3c7c0f5c6a0d3f0ab5c8bcef0adf3be56f5aeed8b2dd1504b7a950fc4fee1f46",
        "utxo_vout": 1,
        "utxo_block_height": 850976,
        "utxo_confirmations": 15735
    }, {
        "inscription_id": "360550a31c9510ed5052c4351619bf68d5ae3f218bf2e9c1092090dbcf86acb3i0",
        "satoshis": "546",
        "utxo_sat_offset": 0,
        "utxo_txid": "3c7c0f5c6a0d3f0ab5c8bcef0adf3be56f5aeed8b2dd1504b7a950fc4fee1f46",
        "utxo_vout": 1,
        "utxo_block_height": 850976,
        "utxo_confirmations": 15735
    }],
    "last_updated": {
        "block_hash": "1c3e7cd9e46bd6d0adb1ae0b52ec1a1ddfaa0cb61a41a1b1c26f0d1b0f42a06c",
        "block_height": 5146710
    },
    "next_cursor": null
}"##;
