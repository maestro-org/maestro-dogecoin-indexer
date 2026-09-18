#[cfg(test)]
pub fn encode(n: u128) -> Vec<u8> {
    let mut v = Vec::new();
    encode_to_vec(n, &mut v);
    v
}

pub fn encode_to_vec(mut n: u128, v: &mut Vec<u8>) {
    let mut out = [0; 19];
    let mut i = 18;

    loop {
        let mut byte = n.to_le_bytes()[0] & 0b0111_1111;

        if i < 18 {
            byte |= 0b1000_0000;
        }

        out[i] = byte;

        if n < 0b1000_0000 {
            break;
        }

        n = n / 128 - 1;
        i -= 1;
    }

    v.extend_from_slice(&out[i..]);
}

pub fn decode(buffer: &[u8]) -> (u128, usize) {
    let mut n = 0;
    let mut i = 0;

    loop {
        let b = match buffer.get(i) {
            Some(b) => u128::from(*b),
            None => return (n, i),
        };

        n = n.saturating_mul(128);

        if b < 128 {
            return (n.saturating_add(b), i + 1);
        }

        n = n.saturating_add(b - 127);

        i += 1;
    }
}
