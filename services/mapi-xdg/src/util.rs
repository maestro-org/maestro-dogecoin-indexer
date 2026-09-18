use std::fmt::{self, Debug};
use std::str::FromStr;
use std::{collections::HashMap, ops::Range};

use dogecoin::hashes::Hash as DHash;
use ordinals::Rune;
use serde::Deserializer;
use timbre_xbt::builder::BREAK;
use timbre_xbt::{
    reducers::{reducer_key_range, script_by_script_hash, script_hash_by_address_payload_hash},
    TimbreError, VarUInt,
};
use timbre_xbt::{Decode, Encode, Prefix, Reducer};

use crate::error::{Error, MapiResult};
use crate::options::Mode;
use crate::polyphony::{PolyphonyWrapper, Snapshot};
use crate::types::{CursorPaginationParams, HeightPaginationParams, OrderParam, Softfork};

pub static DEFAULT_CONTENT_BODY_SIZE: u64 = 100;
pub static MAX_CONTENT_BODY_SIZE: u64 = 4096;
pub static MAX_CONTENT_PREVIEW: usize = 100;
pub static MAX_PAGE_COUNT: usize = 100;

pub fn deserialize_softforks<'de, D>(deserializer: D) -> Result<HashMap<String, Softfork>, D::Error>
where
    D: Deserializer<'de>,
{
    struct SoftforksVisitor;

    impl<'de> serde::de::Visitor<'de> for SoftforksVisitor {
        type Value = HashMap<String, Softfork>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a hashmap or an array of softforks")
        }

        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut map = HashMap::new();
            while let Some((key, value)) = access.next_entry()? {
                map.insert(key, value);
            }
            Ok(map)
        }

        fn visit_seq<S>(self, mut access: S) -> Result<Self::Value, S::Error>
        where
            S: serde::de::SeqAccess<'de>,
        {
            let mut map = HashMap::new();
            let mut count = 0;
            while let Some(value) = access.next_element::<Softfork>()? {
                map.insert(count.to_string(), value);
                count += 1;
            }
            Ok(map)
        }
    }

    deserializer.deserialize_any(SoftforksVisitor)
}

pub struct ParsedHeightPaginationParams {
    count: usize,
    order: OrderParam,
    range: Range<Vec<u8>>,
}

impl ParsedHeightPaginationParams {
    pub fn parse<A: Encode + Decode + Debug + Clone, C: Encode + Decode + Debug>(
        params: HeightPaginationParams,
        encoder: &Prefix,
        reducer: &Reducer,
        key_params: Option<A>,
    ) -> MapiResult<Self> {
        let count = params.count.map(|x| x.0).unwrap_or(MAX_PAGE_COUNT);

        if count > MAX_PAGE_COUNT || count == 0 {
            return Err(Error::MalformedRequest("Invalid page size".into()));
        }

        let order = params.order.unwrap_or(crate::types::OrderParam::Asc);

        // due to key format we need to add 1 to the height x if we want to find
        // keys with height x in the range
        let (mut range_lower, mut range_upper) = reducer_key_range(
            encoder.namespace(),
            reducer,
            &key_params,
            params.from,
            params.to.map(|x| x.saturating_add(1)),
        );

        if let Some(cursor) = &params.cursor {
            let (cursor, _) = C::decode_base64(cursor).map_err(|_| {
                Error::MalformedRequest("Malformed cursor: unable to decode".into())
            })?;

            let mut cursor_key = if let Some(p) = key_params {
                encoder.data(reducer, &(p, BREAK, cursor))
            } else {
                encoder.data(reducer, &cursor)
            };

            if !(range_lower <= cursor_key && cursor_key <= range_upper) {
                return Err(Error::MalformedRequest(
                    "Malformed cursor: invalid for height range".into(),
                ));
            }

            // if ascending, increase cursor key by 1 lexicographically to avoid
            // including cursor kv in scanned keys
            if order == OrderParam::Asc {
                cursor_key.push(0x00);

                range_lower = cursor_key;
            } else {
                range_upper = cursor_key
            }
        }

        Ok(Self {
            count,
            order,
            range: range_lower..range_upper,
        })
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn order(&self) -> OrderParam {
        self.order
    }

    pub fn key_range(&self) -> Range<Vec<u8>> {
        self.range.clone()
    }
}

pub struct ParsedPaginationParams {
    count: usize,
    order: OrderParam,
    range: Range<Vec<u8>>,
}

impl ParsedPaginationParams {
    pub fn parse<A: Encode + Decode + Debug + Clone, C: Encode + Decode + Debug>(
        params: HeightPaginationParams,
        encoder: &Prefix,
        reducer: &Reducer,
        key_params: Option<A>,
    ) -> MapiResult<Self> {
        let count = params.count.map(|x| x.0).unwrap_or(MAX_PAGE_COUNT);

        if count > MAX_PAGE_COUNT || count == 0 {
            return Err(Error::MalformedRequest("Invalid page size".into()));
        }

        let order = params.order.unwrap_or(crate::types::OrderParam::Asc);

        // due to key format we need to add 1 to the height x if we want to find
        // keys with height x in the range
        let (mut range_lower, mut range_upper) = reducer_key_range(
            encoder.namespace(),
            reducer,
            &key_params,
            params.from,
            params.to.map(|x| x.saturating_add(1)),
        );

        if let Some(cursor) = &params.cursor {
            let (cursor, _) = C::decode_base64(cursor).map_err(|_| {
                Error::MalformedRequest("Malformed cursor: unable to decode".into())
            })?;

            let mut cursor_key = if let Some(p) = key_params {
                encoder.data(reducer, &(p, BREAK, cursor))
            } else {
                encoder.data(reducer, &cursor)
            };

            if !(range_lower <= cursor_key && cursor_key <= range_upper) {
                return Err(Error::MalformedRequest(
                    "Malformed cursor: invalid for height range".into(),
                ));
            }

            // if ascending, increase cursor key by 1 lexicographically to avoid
            // including cursor kv in scanned keys
            if order == OrderParam::Asc {
                cursor_key.push(0x00);

                range_lower = cursor_key;
            } else {
                range_upper = cursor_key
            }
        }

        Ok(Self {
            count,
            order,
            range: range_lower..range_upper,
        })
    }

    pub fn parse_no_height<A: Encode + Decode + Debug + Clone, C: Encode + Decode + Debug>(
        params: CursorPaginationParams,
        encoder: &Prefix,
        reducer: &Reducer,
        key_params: Option<A>,
    ) -> MapiResult<Self> {
        let count = params.count.map(|x| x.0).unwrap_or(MAX_PAGE_COUNT);

        if count > MAX_PAGE_COUNT || count == 0 {
            return Err(Error::MalformedRequest("Invalid page size".into()));
        }

        let order = params.order.unwrap_or(crate::types::OrderParam::Asc);

        // due to key format we need to add 1 to the height x if we want to find
        // keys with height x in the range
        let (mut range_lower, mut range_upper) = reducer_key_range(
            encoder.namespace(),
            reducer,
            &key_params,
            None::<u64>,
            None::<u64>,
        );

        if let Some(cursor) = &params.cursor {
            let (cursor, _) = C::decode_base64(cursor).map_err(|_| {
                Error::MalformedRequest("Malformed cursor: unable to decode".into())
            })?;

            let mut cursor_key = if let Some(p) = key_params {
                encoder.data(reducer, &(p, BREAK, cursor))
            } else {
                encoder.data(reducer, &cursor)
            };

            if !(range_lower <= cursor_key && cursor_key <= range_upper) {
                return Err(Error::MalformedRequest(
                    "Malformed cursor: invalid for height range".into(),
                ));
            }

            // if ascending, increase cursor key by 1 lexicographically to avoid
            // including cursor kv in scanned keys
            if order == OrderParam::Asc {
                cursor_key.push(0x00);

                range_lower = cursor_key;
            } else {
                range_upper = cursor_key
            }
        }

        Ok(Self {
            count,
            order,
            range: range_lower..range_upper,
        })
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn order(&self) -> OrderParam {
        self.order
    }

    pub fn key_range(&self) -> Range<Vec<u8>> {
        self.range.clone()
    }
}

pub enum RuneIdentifier {
    Id((u64, u32)),
    Name(u128),
}

impl RuneIdentifier {
    pub fn parse(string: String) -> MapiResult<Self> {
        if string.contains(':') {
            let parts: Vec<_> = string.split(':').collect();
            if parts.len() != 2 {
                return Err(Error::MalformedRequest(
                    "Rune ID must be etching block and transaction index in form '2519999:31'"
                        .into(),
                ));
            }
            let malformed = || {
                Error::MalformedRequest(
                    "Rune ID must be etching block and transaction index in form '2519999:31'"
                        .into(),
                )
            };
            let block = parts[0].parse().map_err(|_| malformed())?;
            let tx = parts[1].parse().map_err(|_| malformed())?;

            Ok(Self::Id((block, tx)))
        } else {
            let without_spacers = string.replace("•", "");

            let rune = Rune::from_str(&without_spacers)
                .map_err(|_| Error::MalformedRequest("Unable to decode rune name".into()))?;

            Ok(Self::Name(rune.n()))
        }
    }
}

pub fn parse_inscription_id(input: &String) -> MapiResult<([u8; 32], u32)> {
    let mut parts = input.split('i');
    let hex_part = parts
        .next()
        .ok_or(Error::MalformedRequest("Empty inscription ID".into()))?;
    let int_part = parts
        .next()
        .ok_or(Error::MalformedRequest("Missing inscription index".into()))?;
    if parts.next().is_some() {
        return Err(Error::MalformedRequest("Wrong inscription ID".into()));
    }

    let mut bytes: [u8; 32] = hex::decode(hex_part)
        .map_err(|_| Error::InvalidHex(hex_part.into()))?
        .try_into()
        .map_err(|_| Error::MalformedRequest("Wrong tx hash in inscription ID".into()))?;

    bytes.reverse();

    let number = int_part
        .parse::<u32>()
        .map_err(|_| Error::MalformedRequest("Wrong index in inscription ID".into()))?;

    Ok((bytes, number))
}

pub fn parse_varuint<T>(val: VarUInt, error_msg: &str) -> MapiResult<T>
where
    T: TryFrom<VarUInt, Error = TimbreError>,
{
    val.try_into()
        .map_err(|e| Error::MalformedData(error_msg.into(), Some(e)))
}

pub enum DogecoinAddress {
    Dogecoin(dogecoin::Address),
}

impl DogecoinAddress {
    pub fn from_str(str: &str) -> MapiResult<Self> {
        if let Ok(a) = dogecoin::Address::from_str(str) {
            return Ok(Self::Dogecoin(a));
        }

        Err(Error::MalformedRequest("Unable to decode address".into()))
    }

    pub fn payload_hash(&self) -> [u8; 20] {
        match self {
            Self::Dogecoin(a) => a.payload.script_pubkey().script_hash().into_inner(),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Self::Dogecoin(a) => a.to_string(),
        }
    }
}

pub async fn parse_address_or_script_bytes(
    polyphony: &PolyphonyWrapper,
    snapshot: &mut Snapshot,
    input: String,
    network: Mode,
) -> MapiResult<(Option<DogecoinAddress>, Vec<u8>)> {
    let (address, script) = match DogecoinAddress::from_str(&input) {
        Ok(addr) => {
            let payload_hash = addr.payload_hash();

            let script_hash = snapshot
                .get_reducer_key_maybe::<_, script_hash_by_address_payload_hash::Value>(
                    &polyphony.script_hash_by_address_payload_hash_encoder()?,
                    &Reducer::ScriptHashByAddressPayloadHash,
                    &script_hash_by_address_payload_hash::Key { payload_hash },
                )
                .await?
                .map(|x| x.script_hash)
                .ok_or_else(|| Error::NotFound)?;

            let script = snapshot
                .get_reducer_key_maybe::<_, script_by_script_hash::Value>(
                    &polyphony.script_by_script_hash_encoder()?,
                    &Reducer::ScriptByScriptHash,
                    &script_by_script_hash::Key { script_hash },
                )
                .await?
                .map(|x| x.script)
                .ok_or_else(|| Error::Internal("missing script by sh".into()))?;

            (Some(addr), script)
        }
        Err(_) => {
            let script_bytes = hex::decode(&input).map_err(|_| {
                Error::MalformedRequest(
                    "Could not decode as address or hex-encoded script pubkey".into(),
                )
            })?;

            match network {
                Mode::Dogecoin => {
                    let script = dogecoin::Script::from(script_bytes);

                    (
                        dogecoin::Address::from_script(&script, dogecoin::Network::Bitcoin)
                            .map(DogecoinAddress::Dogecoin),
                        script.into_bytes(),
                    )
                }
                Mode::DogecoinTestnet => {
                    let script = dogecoin::Script::from(script_bytes);

                    (
                        dogecoin::Address::from_script(&script, dogecoin::Network::Testnet)
                            .map(DogecoinAddress::Dogecoin),
                        script.into_bytes(),
                    )
                }
                Mode::GenerateOpenApi => {
                    unreachable!("server never runs in GenerateOpenApi mode")
                }
            }
        }
    };

    Ok((address, script))
}

pub fn decimal(num: u128, dec: usize) -> String {
    let mut bal_string = num.to_string();
    let bal_string_len = bal_string.len();

    if dec > 0 {
        if bal_string_len == dec {
            let mut new_string = String::from("0.");
            new_string.push_str(&bal_string);

            bal_string = new_string;
        } else if bal_string_len < dec {
            let mut new_string = String::from("0.");

            for _ in 0..(dec - bal_string_len) {
                new_string.push('0')
            }

            new_string.push_str(&bal_string);

            bal_string = new_string;
        } else {
            bal_string.insert(bal_string_len - dec, '.');
        }
    }

    bal_string
}
