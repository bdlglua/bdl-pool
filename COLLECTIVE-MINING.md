# BDL Collective Mining — accounting foundation

**Status: accounting core only. This is not yet a live mining pool.**

This milestone adds a deterministic PPLNS reward-allocation engine to
`bdl-pool-services`. It is deliberately separated from the existing public,
read-only statistics API and does not change the existing BTC or BCH Stratum
services.

## Scope of this first milestone

The accounting core recognises every SHA-256d profile exposed by BDLMinerV2:

- BTC — Bitcoin
- BCH — Bitcoin Cash
- BSV — Bitcoin SV
- XEC — eCash
- DGB — DigiByte SHA-256d
- PPC — Peercoin proof-of-work

It accepts only server-side records for shares that the corresponding mining
backend has already accepted. It uses a PPLNS work window, weights each share by
its assigned difficulty, and allocates the block reward in the coin's smallest
unit. Integer arithmetic and largest-remainder rounding ensure that every
reward atom is assigned exactly once in the plan.

The current implementation is a pure, deterministic library module with unit
tests. It does **not** yet open a Stratum port, accept shares over HTTP, store a
ledger, query coin nodes, detect blocks, create transactions, or sign/broadcast
payments.

## Payout modes

The accounting calculation is identical in both modes. The mode controls what
the future settlement worker is allowed to do with the resulting plan:

| Mode | Settlement behavior |
| --- | --- |
| `automatic` | Queue payouts to each participant's verified address after the block is confirmed and mature, subject to configured minimums and network fees. |
| `manual` | Keep the allocation pending for administrator review and approval; no transaction is signed automatically. |

No payout address, private key, wallet seed, or device secret belongs in the
share record or the public statistics API. Automatic payments will require an
isolated signer/wallet adapter with explicit limits and an auditable payment
journal. Manual mode must use the same validated accounting records and an
administrator approval step.

## Accounting contract

A trusted Stratum adapter will submit an `AcceptedShare` only after the coin
backend validates the proof of work and returns an accepted result. Each record
contains:

- `sequence`: monotonically increasing server-side sequence number;
- `share_id`: unique server-side identifier for idempotency;
- `participant_id`: opaque account identifier, never the wallet address;
- `work_units`: accepted assigned difficulty multiplied by
  `1,000,000`, rounded to an integer by the adapter.

The PPLNS window is measured in work units, not number of shares. The engine
walks backward from the newest accepted share and can include only part of the
oldest share needed to fill the window. When the pool has not yet accumulated a
full window, the engine uses the available accepted work and exposes the actual
amount in `accounted_work_units`.

Each block reward is passed in the corresponding coin's smallest indivisible
unit (`reward_atoms`). The coin adapter must account for the actual spendable
block reward and must not create a plan for an unconfirmed or immature block.
The ledger layer must enforce a unique `(coin, block_id)` key so the same block
cannot be settled twice.

## Next implementation stages

1. **Durable ledger and private coordinator API** — persist participants,
   accepted shares, block candidates, reward plans and payout status with
   idempotency, locking and restricted access. The existing public API remains
   read-only.
2. **Mining backend adapters** — integrate BTC and BCH first, then BSV, XEC,
   DGB and PPC only when each dedicated node and Stratum backend is installed
   and tested. A hashrate dashboard or a Stratum proxy alone is not collective
   mining.
3. **Settlement** — manual review/approval first; automatic payments only after
   wallet isolation, address validation per chain, maturity handling, minimum
   payout and fee policy, and recovery tests.
4. **Firmware integration** — add a persistent opt-in switch and the automatic /
   manual selection only when the coordinator and coin backend advertise that
   the chosen profile is ready. Turning collective mode off leaves the current
   individual pool configuration untouched.

## Safety boundary

The current BTC CKPool and BCH CashStratum instances use solo payout behavior.
They are not silently reconfigured by this module. No service is restarted and
no mining endpoint is changed by adding this accounting foundation.
