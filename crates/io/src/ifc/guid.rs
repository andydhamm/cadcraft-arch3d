//! IFC GlobalIds (`IfcGloballyUniqueId`): 128 bits written as 22 characters of IFC's base-64
//! alphabet. Derived, never random: an entity's id is a hash of a key naming what it stands for, so
//! two exports of one model give the same ids and an object keeps its id while the model changes.
//!
//! Keys: `<kind>/<n>` for an object with `ArchId(n)` (`wall/3`, `opening/0`, `door/0`, `window/2`,
//! `space/1`, `floor/1`, `ceiling/1`, `roof/0`, `roofplane/0/2`, `stair/0`, `railing/0`,
//! `deck/1`, `plant/4`, `path/1`, `light/2`), the singletons `project`, `site`, `building`,
//! `terrain`, `storey/<level index>`, and for relationships and property sets `rel/<what>/<…>` and
//! `pset/<name>/<kind>/<n>` (`rel/voids/0`, `rel/fills/0`, `rel/contains/storey/1`).

/// IFC's base-64 alphabet (buildingSMART, `IfcGloballyUniqueId`).
const ALPHABET: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// The GlobalId for `key`.
pub fn global_id(key: &str) -> String {
    encode(hash128(key))
}

/// 128 bits as 22 characters: the first carries the top 2 bits, each of the other 21 six bits.
pub fn encode(v: u128) -> String {
    let mut out = String::with_capacity(22);
    out.push(ALPHABET[(v >> 126) as usize] as char);
    for i in (0..21).rev() {
        out.push(ALPHABET[((v >> (6 * i)) & 0x3f) as usize] as char);
    }
    out
}

/// The 128 bits of a GlobalId, or `None` when it is not 22 characters of the alphabet with a first
/// character of `0`–`3`.
pub fn decode(id: &str) -> Option<u128> {
    let bytes = id.as_bytes();
    if bytes.len() != 22 {
        return None;
    }
    let mut v: u128 = 0;
    for (i, &b) in bytes.iter().enumerate() {
        let d = ALPHABET.iter().position(|&a| a == b)? as u128;
        if i == 0 && d > 3 {
            return None;
        }
        v = (v << if i == 0 { 2 } else { 6 }) | d;
    }
    Some(v)
}

/// Two FNV-1a passes over the key with different offset bases, each finished with SplitMix64's
/// mixer: stable across platforms and releases (no `std` hasher, whose output may change).
fn hash128(key: &str) -> u128 {
    let fnv = |basis: u64| {
        let mut h = basis;
        for b in key.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        mix(h)
    };
    (u128::from(fnv(0xcbf2_9ce4_8422_2325)) << 64) | u128::from(fnv(0x6c62_272e_07bb_0142))
}

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn a_global_id_is_22_characters_of_the_alphabet() {
        for key in ["project", "wall/0", "rel/voids/12", "Küche"] {
            let id = global_id(key);
            assert_eq!(id.len(), 22, "{id}");
            assert!(id.bytes().all(|b| ALPHABET.contains(&b)), "{id}");
            assert!(matches!(id.as_bytes()[0], b'0'..=b'3'), "{id}: the first character carries 2 bits");
        }
    }

    #[test]
    fn the_same_key_gives_the_same_id_and_different_keys_differ() {
        assert_eq!(global_id("wall/3"), global_id("wall/3"));
        let ids: HashSet<String> = (0..5000).flat_map(|n| [global_id(&format!("wall/{n}")), global_id(&format!("opening/{n}"))]).collect();
        assert_eq!(ids.len(), 10_000);
    }

    #[test]
    fn ids_decode_to_their_bits() {
        for v in [0u128, 1, u128::MAX, 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210] {
            assert_eq!(decode(&encode(v)), Some(v));
        }
        assert_eq!(encode(0), "0000000000000000000000");
        assert_eq!(encode(u128::MAX), "3$$$$$$$$$$$$$$$$$$$$$");
        assert_eq!(decode("4000000000000000000000"), None, "a first character above 3 is more than 128 bits");
        assert_eq!(decode("short"), None);
    }

    #[test]
    fn ids_do_not_change_between_releases() {
        // The hash is part of the file format: a change here gives every exported object a new id.
        // The values come from an independent implementation of the same hash (Python, 2026-11-01).
        assert_eq!(global_id("project"), "2nT9NOAtHv7oiRsTDK1Jl2");
        assert_eq!(global_id("wall/0"), "0AmVA1pQjlMdcMlHacSJ4u");
        assert_eq!(global_id("rel/voids/0"), "3yQIApwTNRTPBvP82eVzcg");
    }
}
