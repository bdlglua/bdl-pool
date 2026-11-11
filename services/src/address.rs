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
    }
}
