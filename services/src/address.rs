use regex::Regex;

pub fn canonical(value: &str, bch: bool) -> Option<String> {
    if !bch {
        let value = if value.starts_with("BC1") && value == value.to_uppercase() {
            value.to_lowercase()
        } else {
            value.into()
        };
        return Regex::new(r"^(?:[13][1-9A-HJ-NP-Za-km-z]{25,34}|bc1[ac-hj-np-z02-9]{11,87})$")
            .unwrap()
            .is_match(&value)
            .then_some(value);
    }
    if value != value.to_lowercase() && value != value.to_uppercase() {
        return None;
    }
    let value = value.to_lowercase();
    let payload = value.strip_prefix("bitcoincash:").unwrap_or(&value);
    if payload.len() != 42 || !matches!(payload.as_bytes()[0], b'q' | b'p') {
        return None;
    }
    let alphabet = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    let values: Option<Vec<u64>> = payload
        .bytes()
        .map(|b| alphabet.iter().position(|c| *c == b).map(|i| i as u64))
        .collect();
    let values = values?;
    let mut checksum = 1_u64;
    for item in b"bitcoincash"
        .iter()
        .map(|c| u64::from(c & 31))
        .chain([0])
        .chain(values.iter().copied())
    {
        let top = checksum >> 35;
        checksum = ((checksum & 0x07ffffffff) << 5) ^ item;
        for (bit, generator) in [
            0x98f2bc8e61,
            0x79b76d99e2,
            0xf33e5fb3c4,
            0xae2eabe2a8,
            0x1e4f43e470,
        ]
        .iter()
        .enumerate()
        {
            if (top >> bit) & 1 != 0 {
                checksum ^= generator;
            }
        }
    }
    let version = (values[0] << 3) | (values[1] >> 2);
    (checksum == 1 && values[33] & 3 == 0 && matches!(version, 0 | 8))
        .then(|| format!("bitcoincash:{payload}"))
}

/// Validate a mainnet payout address for one of the collective SHA-256d coins.
/// This is format and checksum validation; coin-node script validation remains
/// a required check before any transaction is created.
pub fn canonical_coin(value: &str, coin: &str) -> Option<String> {
    match coin {
        "btc" => canonical(value, false),
        "bch" => canonical(value, true),
        "bsv" => base58_version(value, &[0x00, 0x05]),
        "xec" => cashaddr_prefix(value, "ecash"),
        "dgb" => base58_version(value, &[0x1e, 0x3f]),
        "ppc" => base58_version(value, &[0x37, 0x75]),
        _ => None,
    }
}

fn base58_version(value: &str, versions: &[u8]) -> Option<String> {
    use sha2::{Digest, Sha256};
    let raw = bs58::decode(value).into_vec().ok()?;
    if raw.len() != 25 || !versions.contains(&raw[0]) {
        return None;
    }
    let first = Sha256::digest(&raw[..21]);
    let second = Sha256::digest(first);
    (second[..4] == raw[21..]).then(|| value.to_string())
}

fn cashaddr_prefix(value: &str, expected_prefix: &str) -> Option<String> {
    if value != value.to_lowercase() && value != value.to_uppercase() {
        return None;
    }
    let lower = value.to_lowercase();
    let (prefix, payload) = lower.split_once(':')?;
    if prefix != expected_prefix
        || payload.len() != 42
        || !matches!(payload.as_bytes()[0], b'q' | b'p')
    {
        return None;
    }
    let alphabet = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    let values: Option<Vec<u64>> = payload
        .bytes()
        .map(|b| alphabet.iter().position(|c| *c == b).map(|i| i as u64))
        .collect();
    let values = values?;
    let mut checksum = 1_u64;
    for item in prefix
        .bytes()
        .map(|c| u64::from(c & 31))
        .chain([0])
        .chain(values.iter().copied())
    {
        let top = checksum >> 35;
        checksum = ((checksum & 0x07ffffffff) << 5) ^ item;
        for (bit, generator) in [
            0x98f2bc8e61,
            0x79b76d99e2,
            0xf33e5fb3c4,
            0xae2eabe2a8,
            0x1e4f43e470,
        ]
        .iter()
        .enumerate()
        {
            if (top >> bit) & 1 != 0 {
                checksum ^= generator;
            }
        }
    }
    let version = (values[0] << 3) | (values[1] >> 2);
    (checksum == 1 && values[33] & 3 == 0 && matches!(version, 0 | 8)).then_some(lower)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn separates_networks_and_checks_cashaddr() {
        let a = "bitcoincash:qpm2qsznhks23z7629mms6s4cwef74vcwvy22gdx6a";
        assert_eq!(canonical(a, true).as_deref(), Some(a));
        assert_eq!(canonical(&a.to_uppercase(), true).as_deref(), Some(a));
        assert_eq!(canonical(a, false), None);
        assert_eq!(canonical(&a.replace("qpm", "qpn"), true), None);
        assert_eq!(canonical("1BoatSLRHtKNngkdXEeobR76b53LETtpyT", true), None);
        assert_eq!(
            canonical_coin("ecash:qp63uahgrxged4z5jswyt5dn5v3lzsem6cacy2kzvq", "xec").as_deref(),
            Some("ecash:qp63uahgrxged4z5jswyt5dn5v3lzsem6cacy2kzvq")
        );
        assert_eq!(
            canonical_coin("1BoatSLRHtKNngkdXEeobR76b53LETtpyT", "dgb"),
            None
        );
        assert_eq!(canonical_coin("not-an-address", "ppc"), None);
    }
}
