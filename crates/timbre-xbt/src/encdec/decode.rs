use std::hash::Hash;

use base64::{engine::general_purpose as b64, Engine};
use indexmap::IndexMap;

use crate::{CursorValue, Encode, ShortByteString, TimbreError, TimbreVec, VarUInt};

pub trait Decode
where
    Self: Sized,
{
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError>;

    fn decode_base64(s: &str) -> Result<(Self, String), TimbreError> {
        let binding = b64::URL_SAFE_NO_PAD.decode(s)?;
        let (decoded, remaining) = Self::decode(&binding)?;
        let remaining_str = std::str::from_utf8(remaining)?;

        Ok((decoded, remaining_str.to_string()))
    }
}

macro_rules! impl_decode {
    ($t:ty) => {
        impl Decode for $t {
            fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
                let size = std::mem::size_of::<$t>();
                bytes
                    .get(..size)
                    .and_then(|b| Some((<$t>::from_be_bytes(b.try_into().ok()?), &bytes[size..])))
                    .ok_or(TimbreError::MalformedInput(
                        "Insufficient bytes for decoding".to_string(),
                    ))
            }
        }
    };
}

impl_decode!(usize);
impl_decode!(u8);
impl_decode!(u16);
impl_decode!(u32);
impl_decode!(u64);
impl_decode!(u128);
impl_decode!(isize);
impl_decode!(i8);
impl_decode!(i16);
impl_decode!(i32);
impl_decode!(i64);
impl_decode!(i128);

impl Decode for bool {
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        bytes
            .first()
            .map(|b| (*b != 0, &bytes[1..]))
            .ok_or(TimbreError::MalformedInput(
                "Insufficient bytes for bool decoding".to_string(),
            ))
    }
}

impl<const N: usize> Decode for [u8; N] {
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        bytes
            .get(..N)
            .map(|slice| {
                (
                    slice.try_into().expect("slice with incorrect length"),
                    &bytes[N..],
                )
            })
            .ok_or(TimbreError::MalformedInput(
                "Insufficient bytes for array decoding".to_string(),
            ))
    }
}

impl<A: Decode, B: Decode> Decode for (A, B) {
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        let (first, bytes) = A::decode(bytes)?;
        let (second, bytes) = B::decode(bytes)?;

        Ok(((first, second), bytes))
    }
}

impl<A: Decode, B: Decode, C: Decode> Decode for (A, B, C) {
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        let (first, bytes) = A::decode(bytes)?;
        let (second, bytes) = B::decode(bytes)?;
        let (third, bytes) = C::decode(bytes)?;

        Ok(((first, second, third), bytes))
    }
}

impl Decode for ShortByteString {
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        let len = *bytes.first().ok_or(TimbreError::MalformedInput(
            "Empty bytes for ShortByteString decode".into(),
        ))? as usize;
        let (data, bytes) = bytes[1..]
            .split_at_checked(len)
            .ok_or(TimbreError::MalformedInput(
                "Insufficient bytes for ShortByteString decode".into(),
            ))?;

        Ok((ShortByteString(data.to_vec()), bytes))
    }
}

impl Decode for VarUInt {
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        let len = *bytes.first().ok_or(TimbreError::MalformedInput(
            "Empty bytes for VarUInt decode".into(),
        ))? as usize;
        if len > 16 {
            return Err(TimbreError::MalformedInput(
                "VarUInt length byte exceeds 16".into(),
            ));
        }
        let (data, bytes) = bytes[1..]
            .split_at_checked(len)
            .ok_or(TimbreError::MalformedInput(
                "Insufficient bytes for VarUInt decode".into(),
            ))?;

        let be_128: [u8; 16] = [vec![0; 16 - len], data.to_vec()]
            .concat()
            .try_into()
            .unwrap();

        Ok((VarUInt(u128::from_be_bytes(be_128)), bytes))
    }
}

impl Decode for CursorValue {
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        let (height, bytes) = u64::decode(bytes)?;
        let (hash, bytes) = <[u8; 32]>::decode(bytes)?;
        let (was_mempool, bytes) = bool::decode(bytes)?;
        let (timestamp, bytes) = u64::decode(bytes)?;
        let (mempool_info, bytes) = <Option<((u64, [u8; 32]), u64)>>::decode(bytes)?;

        let out = CursorValue {
            height,
            hash,
            was_mempool,
            timestamp,
            mempool_info,
        };

        Ok((out, bytes))
    }
}

impl<T: Decode> Decode for Option<T> {
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        let (presence, bytes) = u8::decode(bytes)?;

        if presence == 0 {
            return Ok((None, bytes));
        }

        let (value, bytes) = T::decode(bytes)?;
        Ok((Some(value), bytes))
    }
}

impl<T: Decode + Encode> Decode for Vec<T> {
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        let (len, mut bytes) = u32::decode(bytes)?;
        let len = len as usize;
        let mut vec = Vec::with_capacity(len);

        for _ in 0..len {
            let (item, rest) = T::decode(bytes)?;
            bytes = rest;
            vec.push(item);
        }

        Ok((vec, bytes))
    }
}

impl<T: Decode + Encode> Decode for TimbreVec<T> {
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        let (len, mut bytes) = VarUInt::decode(bytes)?;
        let len = len.try_into()?;
        let mut vec = Vec::with_capacity(len);

        for _ in 0..len {
            let (item, rest) = T::decode(bytes)?;
            bytes = rest;
            vec.push(item);
        }

        Ok((vec.into(), bytes))
    }
}

impl<K, V> Decode for IndexMap<K, V>
where
    K: Decode + Encode + Eq + Hash,
    V: Decode + Encode + Eq + Hash,
{
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        let mut map = IndexMap::new();

        let (len, mut bytes) = u64::decode(bytes)?; // TODO: make u32
        let len = len as usize;

        for _ in 0..len {
            let (key, rest) = K::decode(bytes)?;
            bytes = rest;
            let (value, rest) = V::decode(bytes)?;
            bytes = rest;
            map.insert(key, value);
        }

        Ok((map, bytes))
    }
}

impl Decode for String {
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        let (len, bytes) = u32::decode(&bytes[..4])?;
        let len = len as usize;
        let (str_bytes, bytes) = bytes.split_at(len);

        Ok((String::from_utf8(str_bytes.to_vec())?, bytes))
    }
}

impl Decode for char {
    fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), TimbreError> {
        if bytes.is_empty() {
            return Err(TimbreError::MalformedInput(
                "Insufficient bytes for char decoding".to_string(),
            ));
        }

        Ok((bytes[0] as char, &bytes[1..]))
    }
}
