use bitcoin::{
    consensus::Decodable,
    opcodes,
    script::{self, Instruction},
    Transaction,
};
use dune::Dune;
use edict::Edict;
use etching::Etching;
use flag::Flag;
use serde::Serialize;
use std::collections::HashMap;
use tag::Tag;
use terms::Terms;

use super::*;

const MAX_SPACERS: u32 = 0b00000111_11111111_11111111_11111111;

#[derive(Default, Serialize, Debug, PartialEq, Clone)]
pub struct Dunestone {
    pub edicts: Vec<Edict>,
    pub etching: Option<Etching>,
    pub pointer: Option<u32>,
    pub cenotaph: bool,
}

struct Message {
    cenotaph: bool,
    fields: HashMap<u128, u128>,
    edicts: Vec<Edict>,
}

impl Message {
    fn from_integers(tx: &Transaction, payload: &[u128]) -> Self {
        let mut edicts = Vec::new();
        let mut fields = HashMap::new();
        let mut cenotaph = false;

        for i in (0..payload.len()).step_by(2) {
            let tag = payload[i];

            if Tag::Body == tag {
                let mut id = 0u128;
                for chunk in payload[i + 1..].chunks_exact(3) {
                    id = id.saturating_add(chunk[0]);
                    if let Some(edict) = Edict::from_integers(tx, id, chunk[1], chunk[2]) {
                        edicts.push(edict);
                    } else {
                        cenotaph = true;
                    }
                }
                break;
            }

            let Some(&value) = payload.get(i + 1) else {
                break;
            };

            fields.entry(tag).or_insert(value);
        }

        Self {
            cenotaph,
            fields,
            edicts,
        }
    }
}

impl Dunestone {
    pub fn from_transaction(transaction: &Transaction) -> Option<Self> {
        Self::decipher(transaction).ok().flatten()
    }

    pub fn from_transaction_bytes(
        transaction_bytes: Vec<u8>,
    ) -> Result<Option<Self>, bitcoin::consensus::encode::Error> {
        Transaction::consensus_decode_from_finite_reader(&mut &transaction_bytes[..])
            .map(|x| Self::from_transaction(&x))
    }

    fn decipher(transaction: &Transaction) -> Result<Option<Self>, script::Error> {
        let Some(payload) = Dunestone::payload(transaction)? else {
            return Ok(None);
        };

        let integers = Dunestone::integers(&payload);

        let Message {
            cenotaph,
            mut fields,
            mut edicts,
        } = Message::from_integers(transaction, &integers);

        /* Ignore deadline
        let deadline = Tag::Deadline
            .take(&mut fields)
            .and_then(|deadline| u32::try_from(deadline).ok());*/

        let pointer = Tag::Pointer
            .take(&mut fields)
            .and_then(|default| u32::try_from(default).ok());

        let divisibility = Tag::Divisibility
            .take(&mut fields)
            .and_then(|divisibility| u8::try_from(divisibility).ok())
            .and_then(|divisibility| (divisibility <= MAX_DIVISIBILITY).then_some(divisibility));

        let limit = Tag::Limit
            .take(&mut fields)
            .map(|limit| limit.clamp(0, MAX_LIMIT));

        let dune = Tag::Dune.take(&mut fields).map(Dune);

        let cap = Tag::Cap.take(&mut fields).map(|cap| cap);

        let premine = Tag::Premine.take(&mut fields).map(|premine| premine);

        if premine.unwrap_or_default() > 0 {
            edicts.push(Edict {
                id: 0,
                amount: premine.unwrap_or_default(),
                output: 1,
            });
        }

        let spacers = Tag::Spacers
            .take(&mut fields)
            .and_then(|spacers| u32::try_from(spacers).ok())
            .and_then(|spacers| (spacers <= MAX_SPACERS).then_some(spacers));

        let symbol = Tag::Symbol
            .take(&mut fields)
            .and_then(|symbol| u32::try_from(symbol).ok())
            .and_then(char::from_u32);

        let height = (
            Tag::HeightStart
                .take(&mut fields)
                .and_then(|start_height| u64::try_from(start_height).ok()),
            Tag::HeightEnd
                .take(&mut fields)
                .and_then(|end_height| u64::try_from(end_height).ok()),
        );

        let offset = (
            Tag::OffsetStart
                .take(&mut fields)
                .and_then(|start_offset| u64::try_from(start_offset).ok()),
            Tag::OffsetEnd
                .take(&mut fields)
                .and_then(|end_offset| u64::try_from(end_offset).ok()),
        );

        let mut flags = Tag::Flags.take(&mut fields).unwrap_or_default();

        let etch = Flag::Etching.take(&mut flags);

        let terms = Flag::Terms.take(&mut flags);

        let turbo = Flag::Turbo.take(&mut flags);

        let overflow = (|| {
            let premine = premine.unwrap_or_default();
            let cap = cap.unwrap_or_default();
            let limit = limit.unwrap_or_default();
            premine.checked_add(cap.checked_mul(limit)?)
        })()
        .is_none();

        let etching = if etch {
            Some(Etching {
                divisibility,
                dune,
                spacers,
                symbol,
                terms: terms.then_some(Terms {
                    cap,
                    height,
                    limit,
                    offset,
                }),
                premine,
                turbo,
            })
        } else {
            None
        };

        Ok(Some(Self {
            cenotaph: cenotaph || overflow || flags != 0 || fields.keys().any(|tag| tag % 2 == 0),
            pointer,
            edicts,
            etching,
        }))
    }

    fn payload(transaction: &Transaction) -> Result<Option<Vec<u8>>, script::Error> {
        for output in &transaction.output {
            let mut instructions = output.script_pubkey.instructions();

            if instructions.next().transpose()? != Some(Instruction::Op(opcodes::all::OP_RETURN)) {
                continue;
            }

            if instructions.next().transpose()? != Some(Instruction::PushBytes(b"D".into())) {
                continue;
            }

            let mut payload = Vec::new();

            for result in instructions {
                if let Instruction::PushBytes(push) = result? {
                    payload.extend_from_slice(push.as_bytes());
                }
            }

            return Ok(Some(payload));
        }

        Ok(None)
    }

    fn integers(payload: &[u8]) -> Vec<u128> {
        let mut integers = Vec::new();
        let mut i = 0;

        while i < payload.len() {
            let (integer, length) = varint::decode(&payload[i..]);
            integers.push(integer);
            i += length;
        }

        integers
    }
}
