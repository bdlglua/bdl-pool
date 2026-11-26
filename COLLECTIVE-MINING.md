# BDL Collective Mining — coordinator foundation

**Status: accounting core and durable coordinator API are implemented. This is not yet a live collective mining pool.**

This milestone adds a deterministic PPLNS reward-allocation engine, a private coordinator API and a durable append-only share ledger. It is separate from the existing public, read-only statistics API and does not change the BTC or BCH Stratum services.

## Supported coin profiles

The accounting core recognises all six SHA-256d profiles exposed by BDLMinerV2:

- BTC — Bitcoin
- BCH — Bitcoin Cash
- BSV — Bitcoin SV
- XEC — eCash
- DGB — DigiByte SHA-256d
- PPC — Peercoin proof-of-work

Payout address formats are checked for the selected mainnet profile. The matching coin-node adapter must still validate the destination script before any future transaction is built.

## Current implementation

The repository now includes:

- A PPLNS accounting engine. Each accepted share is weighted by its assigned difficulty; integer largest-remainder rounding allocates every reward atom exactly once.
- A private coordinator service bound only to 127.0.0.1:8082.
- A locked append-only journal. Each event is appended and synced before the API confirms success. The journal stores participant records, accepted shares, payout-mode changes and mature-block allocation plans.
- Separate administrator, backend and participant bearer tokens. Participant tokens are stored as SHA-256 hashes and only grant access to that participant's own summary.
- Server-assigned monotonic share sequence numbers and duplicate-share protection.
- Idempotent block recording by coin and block identifier.
- An explicit maturity gate: block allocation requests are rejected unless the trusted backend marks the block as mature.

The coordinator does **not** yet connect to a Stratum listener or a coin node. No miner currently sends work to it automatically. It does not detect blocks, create transactions, sign or broadcast payments. The public status response therefore reports collectiveMiningReady=false.

## Payout modes

The PPLNS calculation is identical in both modes. The selected mode controls the future settlement worker:

| Mode | Intended settlement |
| --- | --- |
| automatic | Queue transfers to each participant's validated address after the block is mature, subject to minimums and network fees. |
| manual | Keep allocations pending for administrator review and approval. No transaction is signed automatically. |

The default mode is manual. The coordinator can record both modes, but neither mode currently sends payments. Automatic payments require an isolated signer/wallet adapter, spending limits, an auditable payment journal, per-chain output construction and recovery tests.

## Install the coordinator service

On the existing pool server, as the unprivileged bdl account, from the repository directory:

    cd /home/bdl/bdl-pool-site
    bash api/collective-install.sh

The installer builds the Rust services, creates a private configuration once, prints two random 256-bit secrets once, installs a separate loopback-only systemd service, and adds a dedicated Caddy HTTPS route. Re-running the installer does not replace existing tokens.

Save ADMIN_TOKEN and BACKEND_TOKEN in a password manager. The backend token must remain on the server and must never be embedded in firmware, the public website or a miner configuration. The admin token is only for operator actions. Each participant receives a separate read-only token at enrolment.

The installer does not restart Bitcoin Core, CKPool, BCHN or CashStratum and does not alter their configuration.

## Private coordinator API

Base path: https://api.bdl-pool.com:36018/api/v1/collective

All write requests require Content-Type: application/json and a bearer token. Responses are marked no-store. The existing public statistics API remains read-only.

| Route | Access | Purpose |
| --- | --- | --- |
| GET /status | Public, read-only | Aggregate participants and accepted work by coin; no payout addresses or tokens. |
| POST /admin/participants | Admin token | Enrol a participant and return its one-time read-only participant token. |
| POST /admin/payout-mode | Admin token | Select automatic or manual; default is manual. |
| GET /admin/participants | Admin token | List participant identities and payout addresses. |
| GET /admin/blocks | Admin token | Inspect prepared allocations. |
| GET /me | Participant token | Read the participant's own shares, accepted work and pending reward atoms. |
| POST /internal/shares | Backend token only | Append a share already accepted by a trusted coin-specific Stratum backend. |
| POST /internal/blocks | Backend token only | Prepare an allocation only when the trusted coin backend reports mature=true. |

Participant enrolment requires the administrator token. The participant token is shown only once and must be saved securely by the user/device. The API currently provides no public self-enrolment.

The internal share endpoint is not a simulated mining endpoint. It must only be called by a trusted Stratum adapter after the coin backend has verified and accepted the share. The share sequence is allocated by the server; clients cannot choose it. A repeated (coin, shareId) is rejected.

The block endpoint must only be called by a trusted backend after the node confirms that the block is mature. The same (coin, blockId) cannot be recorded twice with different reward data. The resulting allocation remains awaiting_settlement and paymentsEnabled=false.

Example operator calls, with the tokens supplied from the local password manager:

    curl -fsS https://api.bdl-pool.com:36018/api/v1/collective/status

    curl -fsS -X POST \
      -H "Authorization: Bearer $ADMIN_TOKEN" \
      -H "Content-Type: application/json" \
      -d '{"coin":"btc","payoutAddress":"YOUR_VALID_BTC_MAINNET_ADDRESS"}' \
      https://api.bdl-pool.com:36018/api/v1/collective/admin/participants

    curl -fsS -X POST \
      -H "Authorization: Bearer $ADMIN_TOKEN" \
      -H "Content-Type: application/json" \
      -d '{"payoutMode":"manual"}' \
      https://api.bdl-pool.com:36018/api/v1/collective/admin/payout-mode

Do not commit tokens or participant addresses to Git.

## Accounting contract

A trusted Stratum adapter records a share only after the coin backend validates the proof of work and returns an accepted result. Each record contains:

- sequence: monotonically increasing server-side sequence number;
- shareId: unique server-side identifier for idempotency;
- participantId: opaque participant identifier, never the wallet address;
- workUnits: assigned share difficulty multiplied by 1,000,000 and rounded to an integer by the adapter.

The PPLNS window is measured in work units, not number of shares. The engine walks backwards from the newest accepted share and can include only part of the oldest share needed to fill the window. If history is shorter than the requested window, the engine uses the available accepted work and exposes the actual amount.

The block reward is passed in the corresponding coin's smallest indivisible unit (rewardAtoms). The coin adapter must supply the actual spendable reward and must not create an allocation plan for an unconfirmed or immature block.

## Remaining implementation stages

1. **Stratum backend adapters.** Route BDLMinerV2 devices to a shared BTC/BCH pool backend first, then add BSV, XEC, DGB and PPC only with their own tested node/template backends. Authenticate each worker, verify shares and call the private API only after an accepted result.
2. **Node and block integration.** Connect node RPC, identify block candidates and track confirmation/maturity rules separately for each coin.
3. **Settlement.** Implement manual approval and payout export first. Enable automatic payments only after signer isolation, per-chain transaction construction, address/script checks, fee/minimum limits, replay protection and recovery tests.
4. **Firmware integration.** Add a persistent opt-in switch, participant token storage, payout address per coin and the automatic/manual view only after the selected backend advertises readiness. Turning collective mode off must leave the current individual pool configuration untouched.
5. **Dashboard.** Show participants, accepted work, contribution, mature blocks, pending allocations and completed transactions without exposing other participants' payout addresses.

## Safety boundary

The current BTC CKPool and BCH CashStratum instances still use solo payout behaviour. The coordinator is not advertised as ready and it will not send payments. It is a separate service, not a conversion of existing solo mining into pooled mining. A hashrate dashboard or a Stratum proxy alone is not collective mining.
