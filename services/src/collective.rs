//! Reward accounting primitives for the BDL collective-mining pool.
//!
//! This module deliberately does not accept shares from miners or sign/send
//! transactions. A trusted Stratum adapter must call it only after the share
//! has been validated and accepted by the coin's pool backend. A separate
//! settlement worker will later turn the resulting plan into on-chain payouts.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// Supported SHA-256d profiles exposed by BDLMinerV2.
pub const SUPPORTED_COINS: [&str; 6] = ["btc", "bch", "bsv", "xec", "dgb", "ppc"];

/// One difficulty point is represented by one million work units.
/// For example, an accepted share at difficulty 0.0002 is 200 work units.
pub const WORK_UNITS_PER_DIFFICULTY: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayoutMode {
    /// Queue transfers to each participant's verified payout address.
    Automatic,
    /// Keep a pending distribution for explicit administrator approval.
    Manual,
}

impl PayoutMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Manual => "manual",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "automatic" => Some(Self::Automatic),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// A share that has already been accepted by the upstream mining backend.
///
/// `sequence` is a monotonically increasing server-side sequence, not a
/// timestamp supplied by the miner. `participant_id` is an opaque account ID;
/// it must not be a payout address, API key, or device secret.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedShare {
    pub sequence: u64,
    pub share_id: String,
    pub participant_id: String,
    /// Assigned share difficulty in micro-difficulty work units.
    pub work_units: u64,
}

/// One participant's allocation, in the coin's smallest indivisible unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayoutAllocation {
    pub participant_id: String,
    pub work_units: u128,
    pub amount_atoms: u64,
}

/// A deterministic PPLNS allocation plan for one confirmed block reward.
///
/// The plan is accounting output only. It is not evidence that the block is
/// mature, and it does not broadcast a payment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardPlan {
    pub coin: String,
    pub block_id: String,
    pub payout_mode: PayoutMode,
    pub reward_atoms: u64,
    /// Requested PPLNS window, in work units.
    pub requested_window_work_units: u128,
    /// Actual work included; can be smaller while the pool is new.
    pub accounted_work_units: u128,
    pub payouts: Vec<PayoutAllocation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccountingError {
    UnsupportedCoin,
    InvalidBlockId,
    InvalidReward,
    InvalidWindow,
    InvalidShare,
    DuplicateShareId,
    DuplicateSequence,
    NoEligibleShares,
    ArithmeticOverflow,
}

impl fmt::Display for AccountingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnsupportedCoin => "unsupported coin profile",
            Self::InvalidBlockId => "invalid block identifier",
            Self::InvalidReward => "block reward must be greater than zero",
            Self::InvalidWindow => "PPLNS work window must be greater than zero",
            Self::InvalidShare => "invalid accepted share record",
            Self::DuplicateShareId => "duplicate accepted share identifier",
            Self::DuplicateSequence => "duplicate accepted-share sequence",
            Self::NoEligibleShares => "no accepted shares in the PPLNS window",
            Self::ArithmeticOverflow => "reward accounting overflow",
        };
        f.write_str(message)
    }
}

impl std::error::Error for AccountingError {}

fn valid_identifier(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':')
        })
}

/// Builds a PPLNS reward plan using the newest accepted work up to the
/// requested difficulty window. The oldest included share can be partially
/// counted, so the selected work equals the window when enough history exists.
///
/// Rounding uses the largest-remainder method. Every atom is assigned and the
/// final allocations always sum exactly to `reward_atoms`.
pub fn plan_pplns_reward(
    coin: &str,
    block_id: &str,
    reward_atoms: u64,
    payout_mode: PayoutMode,
    requested_window_work_units: u128,
    accepted_shares: &[AcceptedShare],
) -> Result<RewardPlan, AccountingError> {
    if !SUPPORTED_COINS.contains(&coin) {
        return Err(AccountingError::UnsupportedCoin);
    }
    if !valid_identifier(block_id, 128) {
        return Err(AccountingError::InvalidBlockId);
    }
    if reward_atoms == 0 {
        return Err(AccountingError::InvalidReward);
    }
    if requested_window_work_units == 0 {
        return Err(AccountingError::InvalidWindow);
    }

    let mut ids = BTreeSet::new();
    let mut sequences = BTreeSet::new();
    for share in accepted_shares {
        if share.sequence == 0
            || share.work_units == 0
            || !valid_identifier(&share.share_id, 128)
            || !valid_identifier(&share.participant_id, 96)
        {
            return Err(AccountingError::InvalidShare);
        }
        if !ids.insert(share.share_id.as_str()) {
            return Err(AccountingError::DuplicateShareId);
        }
        if !sequences.insert(share.sequence) {
            return Err(AccountingError::DuplicateSequence);
        }
    }

    let mut ordered: Vec<&AcceptedShare> = accepted_shares.iter().collect();
    ordered.sort_by_key(|share| share.sequence);

    // Keep an exact difficulty window, walking backwards from the newest share.
    let mut remaining = requested_window_work_units;
    let mut weights: BTreeMap<String, u128> = BTreeMap::new();
    for share in ordered.into_iter().rev() {
        if remaining == 0 {
            break;
        }
        let included = remaining.min(share.work_units as u128);
        let entry = weights.entry(share.participant_id.clone()).or_default();
        *entry = entry
            .checked_add(included)
            .ok_or(AccountingError::ArithmeticOverflow)?;
        remaining -= included;
    }

    let accounted_work_units = requested_window_work_units - remaining;
    if accounted_work_units == 0 {
        return Err(AccountingError::NoEligibleShares);
    }

    let total_work = weights.values().try_fold(0_u128, |sum, weight| {
        sum.checked_add(*weight)
            .ok_or(AccountingError::ArithmeticOverflow)
    })?;
    if total_work == 0 {
        return Err(AccountingError::NoEligibleShares);
    }

    // Floor each proportional allocation, then distribute the remaining atoms
    // by descending fractional remainder (participant ID breaks ties).
    let mut rows: Vec<(String, u128, u64, u128)> = Vec::with_capacity(weights.len());
    let mut allocated = 0_u64;
    for (participant_id, work_units) in weights {
        let numerator = (reward_atoms as u128)
            .checked_mul(work_units)
            .ok_or(AccountingError::ArithmeticOverflow)?;
        let base = numerator / total_work;
        let remainder = numerator % total_work;
        let amount_atoms = u64::try_from(base).map_err(|_| AccountingError::ArithmeticOverflow)?;
        allocated = allocated
            .checked_add(amount_atoms)
            .ok_or(AccountingError::ArithmeticOverflow)?;
        rows.push((participant_id, work_units, amount_atoms, remainder));
    }

    let leftover = reward_atoms
        .checked_sub(allocated)
        .ok_or(AccountingError::ArithmeticOverflow)? as usize;
    let mut remainder_order: Vec<usize> = (0..rows.len()).collect();
    remainder_order.sort_by(|a, b| {
        rows[*b]
            .3
            .cmp(&rows[*a].3)
            .then_with(|| rows[*a].0.cmp(&rows[*b].0))
    });
    if leftover > remainder_order.len() {
        return Err(AccountingError::ArithmeticOverflow);
    }
    for index in remainder_order.into_iter().take(leftover) {
        let next = rows[index]
            .2
            .checked_add(1)
            .ok_or(AccountingError::ArithmeticOverflow)?;
        rows[index].2 = next;
    }

    let payouts = rows
        .into_iter()
        .map(|(participant_id, work_units, amount_atoms, _)| PayoutAllocation {
            participant_id,
            work_units,
            amount_atoms,
        })
        .collect::<Vec<_>>();

    let distributed = payouts.iter().try_fold(0_u64, |sum, payout| {
        sum.checked_add(payout.amount_atoms)
            .ok_or(AccountingError::ArithmeticOverflow)
    })?;
    if distributed != reward_atoms {
        return Err(AccountingError::ArithmeticOverflow);
    }

    Ok(RewardPlan {
        coin: coin.to_string(),
        block_id: block_id.to_string(),
        payout_mode,
        reward_atoms,
        requested_window_work_units,
        accounted_work_units,
        payouts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share(sequence: u64, id: &str, participant: &str, work_units: u64) -> AcceptedShare {
        AcceptedShare {
            sequence,
            share_id: id.into(),
            participant_id: participant.into(),
            work_units,
        }
    }

    #[test]
    fn pplns_uses_newest_work_and_partially_counts_oldest_included_share() {
        let shares = [
            share(1, "s1", "alice", 70),
            share(2, "s2", "bob", 30),
            share(3, "s3", "alice", 40),
            share(4, "s4", "carol", 60),
        ];
        let plan = plan_pplns_reward(
            "btc", "block-1", 101, PayoutMode::Manual, 100, &shares,
        ).unwrap();

        assert_eq!(plan.accounted_work_units, 100);
        assert_eq!(plan.payouts.len(), 2);
        assert_eq!(plan.payouts[0], PayoutAllocation {
            participant_id: "alice".into(), work_units: 40, amount_atoms: 40,
        });
        assert_eq!(plan.payouts[1], PayoutAllocation {
            participant_id: "carol".into(), work_units: 60, amount_atoms: 61,
        });
        assert_eq!(plan.payouts.iter().map(|p| p.amount_atoms).sum::<u64>(), 101);
    }

    #[test]
    fn largest_remainder_rounding_is_exact_and_deterministic() {
        let shares = [
            share(1, "s1", "charlie", 1),
            share(2, "s2", "alice", 1),
            share(3, "s3", "bob", 1),
        ];
        let plan = plan_pplns_reward(
            "bch", "block-2", 2, PayoutMode::Automatic, 3, &shares,
        ).unwrap();

        assert_eq!(plan.payouts, vec![
            PayoutAllocation { participant_id: "alice".into(), work_units: 1, amount_atoms: 1 },
            PayoutAllocation { participant_id: "bob".into(), work_units: 1, amount_atoms: 1 },
            PayoutAllocation { participant_id: "charlie".into(), work_units: 1, amount_atoms: 0 },
        ]);
    }

    #[test]
    fn payout_mode_changes_settlement_instruction_not_the_math() {
        let shares = [share(1, "s1", "alice", 2), share(2, "s2", "bob", 3)];
        let automatic = plan_pplns_reward(
            "ppc", "block-3", 100, PayoutMode::Automatic, 5, &shares,
        ).unwrap();
        let manual = plan_pplns_reward(
            "ppc", "block-3", 100, PayoutMode::Manual, 5, &shares,
        ).unwrap();

        assert_ne!(automatic.payout_mode, manual.payout_mode);
        assert_eq!(automatic.payouts, manual.payouts);
    }

    #[test]
    fn all_firmware_coin_profiles_are_recognized_by_the_accounting_core() {
        for coin in SUPPORTED_COINS {
            let plan = plan_pplns_reward(
                coin, "block-4", 10, PayoutMode::Manual, 1, &[share(1, "s1", "miner-1", 1)],
            ).unwrap();
            assert_eq!(plan.coin, coin);
        }
    }

    #[test]
    fn insufficient_history_uses_only_available_accepted_work() {
        let plan = plan_pplns_reward(
            "dgb", "block-5", 9, PayoutMode::Manual, 100, &[share(1, "s1", "alice", 25)],
        ).unwrap();

        assert_eq!(plan.requested_window_work_units, 100);
        assert_eq!(plan.accounted_work_units, 25);
        assert_eq!(plan.payouts[0].work_units, 25);
        assert_eq!(plan.payouts[0].amount_atoms, 9);
    }

    #[test]
    fn unsupported_coin_and_duplicate_shares_are_rejected() {
        let one = [share(1, "s1", "alice", 1)];
        assert_eq!(
            plan_pplns_reward("ltc", "block-6", 1, PayoutMode::Manual, 1, &one),
            Err(AccountingError::UnsupportedCoin)
        );
        let duplicate_id = [share(1, "same", "alice", 1), share(2, "same", "bob", 1)];
        assert_eq!(
            plan_pplns_reward("btc", "block-6", 1, PayoutMode::Manual, 1, &duplicate_id),
            Err(AccountingError::DuplicateShareId)
        );
        let duplicate_sequence = [share(1, "s1", "alice", 1), share(1, "s2", "bob", 1)];
        assert_eq!(
            plan_pplns_reward("btc", "block-6", 1, PayoutMode::Manual, 1, &duplicate_sequence),
            Err(AccountingError::DuplicateSequence)
        );
    }

    #[test]
    fn malformed_and_empty_inputs_are_rejected() {
        assert_eq!(
            plan_pplns_reward("btc", "block 1", 1, PayoutMode::Manual, 1, &[]),
            Err(AccountingError::InvalidBlockId)
        );
        assert_eq!(
            plan_pplns_reward("btc", "block-1", 1, PayoutMode::Manual, 1, &[]),
            Err(AccountingError::NoEligibleShares)
        );
        assert_eq!(
            plan_pplns_reward(
                "btc", "block-1", 1, PayoutMode::Manual, 1, &[share(1, "s1", "alice", 0)],
            ),
            Err(AccountingError::InvalidShare)
        );
    }
}
