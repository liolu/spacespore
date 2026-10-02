//! Graine du monde sous forme de code court et partageable (ex. `K7Q2-M9XA`).
//!
//! Seul l'affichage change : la graine reste un nombre sur 64 bits. Le code utilise le même
//! alphabet que les codes d'invitation (base 32 de Crockford, sans I, L, O ni U). La graine est
//! d'abord mélangée de façon réversible, pour que des graines voisines (42, 43…) donnent des codes
//! qui ne se ressemblent pas. Graine < 2^40 : 8 caractères ; au-delà : 13.

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const SHORT_BITS: u32 = 40;
const SHORT_LEN: usize = 8;
const LONG_LEN: usize = 13;
const K1: u64 = 0xD6E8_FEB8_6659_FD93;
const K2: u64 = 0xA3B1_9535_4A39_B70D;

fn mask(bits: u32) -> u64 {
    if bits >= 64 { u64::MAX } else { (1u64 << bits) - 1 }
}

/// Inverse d'un nombre impair modulo 2^64 (méthode de Newton).
const fn odd_inverse(k: u64) -> u64 {
    let mut inv = k;
    let mut i = 0;
    while i < 6 {
        inv = inv.wrapping_mul(2u64.wrapping_sub(k.wrapping_mul(inv)));
        i += 1;
    }
    inv
}

fn unxorshift(y: u64, s: u32, bits: u32) -> u64 {
    let mut x = y;
    for _ in 0..bits / s + 1 {
        x = y ^ (x >> s);
    }
    x
}

/// Mélange bijectif sur `bits` bits.
fn scramble(x: u64, bits: u32) -> u64 {
    let m = mask(bits);
    let mut x = x & m;
    x ^= x >> 17;
    x = x.wrapping_mul(K1) & m;
    x ^= x >> 13;
    x = x.wrapping_mul(K2) & m;
    x ^ (x >> 16)
}

fn unscramble(x: u64, bits: u32) -> u64 {
    let m = mask(bits);
    let mut x = unxorshift(x & m, 16, bits);
    x = x.wrapping_mul(odd_inverse(K2)) & m;
    x = unxorshift(x, 13, bits);
    x = x.wrapping_mul(odd_inverse(K1)) & m;
    unxorshift(x, 17, bits)
}

/// Code court d'une graine, par groupes de 4 caractères : `K7Q2-M9XA`.
pub fn encode(seed: u64) -> String {
    let (value, len) = if seed <= mask(SHORT_BITS) {
        (scramble(seed, SHORT_BITS), SHORT_LEN)
    } else {
        (scramble(seed, 64), LONG_LEN)
    };
    let mut out = String::with_capacity(len + len / 4);
    for i in 0..len {
        if i > 0 && i % 4 == 0 {
            out.push('-');
        }
        let shift = 5 * (len - 1 - i) as u32;
        let digit = if shift >= 64 { 0 } else { (value >> shift) & 31 };
        out.push(ALPHABET[digit as usize] as char);
    }
    out
}

/// Graine d'un code court (tirets, espaces et casse ignorés ; O → 0, I et L → 1).
pub fn decode(code: &str) -> Option<u64> {
    let mut value: u128 = 0;
    let mut len = 0;
    for c in code.chars() {
        if c == '-' || c == ' ' {
            continue;
        }
        let c = match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        };
        let digit = ALPHABET.iter().position(|&a| a as char == c)? as u128;
        value = (value << 5) | digit;
        len += 1;
        if len > LONG_LEN {
            return None;
        }
    }
    match len {
        SHORT_LEN => Some(unscramble(value as u64, SHORT_BITS)),
        LONG_LEN if value <= u64::MAX as u128 => {
            let seed = unscramble(value as u64, 64);
            // Une petite graine a toujours un code de 8 caractères
            (seed > mask(SHORT_BITS)).then_some(seed)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip() {
        let mut x = 0x1234_5678_9ABC_DEF0u64;
        let mut seeds = vec![0, 1, 42, 43, 1 << 39, mask(SHORT_BITS), mask(SHORT_BITS) + 1, u64::MAX];
        for _ in 0..2000 {
            x = crate::planetgen::seeds::splitmix64(x);
            seeds.push(x);
            seeds.push(x >> 30);
        }
        for seed in seeds {
            let code = encode(seed);
            assert_eq!(decode(&code), Some(seed), "{seed} -> {code}");
            assert_eq!(decode(&code.to_lowercase().replace('-', " ")), Some(seed));
        }
    }

    #[test]
    fn the_default_seed_has_a_short_code() {
        let code = encode(crate::settings::DEFAULT_WORLD_SEED);
        assert_eq!(code.len(), 9, "{code}");
        assert_eq!(code.as_bytes()[4], b'-');
        // Des graines voisines donnent des codes bien différents
        let other = encode(crate::settings::DEFAULT_WORLD_SEED + 1);
        let same = code.chars().zip(other.chars()).filter(|(a, b)| a == b).count();
        assert!(same <= 4, "{code} / {other}");
        assert_eq!(encode(u64::MAX).len(), 16);
    }

    #[test]
    fn bad_codes_are_rejected() {
        for bad in ["", "ABC", "K7Q2-M9XU", "K7Q2-M9XA-1", "ZZZZ-ZZZZ-ZZZZ-Z", "!!!!-????"] {
            assert_eq!(decode(bad), None, "{bad}");
        }
    }
}
