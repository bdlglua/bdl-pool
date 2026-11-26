# BDL Collective Mining

**Status: the coordinator, the first BTC Stratum V1 backend and the Bitcoin Core candidate/maturity watcher are implemented in code. They have not been installed on the production pool server. Participant payouts and firmware opt-in are not implemented.**

This project adds a self-hosted collective-mining backend for the SHA-256d coin profiles exposed by BDLMinerV2. It is separate from anonymous hashrate/statistics sharing. The existing public statistics API remains read-only.

## Supported coin profiles

The accounting engine recognises all six current BDLMinerV2 SHA-256d profiles:

- BTC — Bitcoin
- BCH — Bitcoin Cash
- BSV — Bitcoin SV
- XEC — eCash
- DGB — DigiByte SHA-256d
- PPC — Peercoin proof-of-work

The first pool backend targets BTC over Stratum V1. Other coin-specific Stratum/node adapters are not implemented yet.

## Implemented components

- Deterministic PPLNS accounting for all six profiles. Accepted work is weighted by assigned share difficulty, and largest-remainder rounding allocates the complete reward in the coin's smallest unit.
- A private coordinator bound to 127.0.0.1:8082, with a durable append-only journal and a single-process file lock.
- Separate administrator, backend, participant-dashboard and Stratum-mining credentials.
- A BTC Stratum V1 authentication proxy intended for 127.0.0.1:3337. It verifies the participant ID and dedicated Stratum password before forwarding mining.authorize.
- A separate pool-mode CKPool instance intended for 127.0.0.1:3338. It uses the existing local Bitcoin Core RPC configuration but has its own common BTC payout address, listener, mining IPC socket, runtime directory and share-log directory.
- A CKPool share-log collector. It reads only records with result=true, converts the assigned diff field to work units, and submits accepted work using the server-only backend token. A persistent cursor and coordinator-side duplicate IDs protect normal restarts.
- A Bitcoin Core watcher that scans newly found blocks, checks the coinbase output to the configured collective address, and stores a block candidate with the coordinator's current accepted-share sequence cutoff.
- The watcher waits for 100 confirmations before asking the coordinator to create the PPLNS allocation. It rechecks the canonical block hash and reward, records orphaned candidates and rewinds the scan after a reorganisation.
- A dedicated OpenPort profile named collective, forwarded to local port 3337. It can be installed without restarting the existing V1, V2 or HTTPS relays.

The coordinator's public status still reports collectiveMiningReady=false. The BTC server-side path is implemented, but participant self-enrolment, firmware controls, a user dashboard, manual settlement and payment broadcasting are still missing. No production service has been installed and no real miners have been switched to it.

## Credentials and participant enrolment

Participant enrolment through the admin API returns two distinct one-time secrets:

- participantToken: read-only access to that participant's private dashboard summary.
- stratumPassword: authorizes mining through the BTC Stratum V1 proxy. It is stored as a hash in the coordinator and is not interchangeable with participantToken.

The BTC Stratum username is the 32-character participant ID, optionally followed by a worker suffix, for example:

    0123456789abcdef0123456789abcdef.gamma601

Use stratumPassword as the Stratum V1 password. Never put ADMIN_TOKEN or BACKEND_TOKEN in firmware. BACKEND_TOKEN stays on the server and is used only by trusted services.

## Coordinator installation

On the pool server, run as the unprivileged bdl account:

    cd /home/bdl/bdl-pool-site
    bash api/collective-install.sh

The installer creates the private coordinator config once, generates separate random 256-bit admin/backend tokens, installs a loopback-only service and adds the dedicated HTTPS API route. Re-running it does not replace existing tokens.

Save ADMIN_TOKEN and BACKEND_TOKEN in a password manager. The backend token must remain on the server. The participant API token and Stratum password are returned once when an account is enrolled; save each one securely.

## Private coordinator API

Base path:

    https://api.bdl-pool.com:36018/api/v1/collective

All write requests require Content-Type: application/json and a bearer token. Responses are marked no-store. The existing public statistics API remains read-only.

| Route | Access | Purpose |
| --- | --- | --- |
| GET /status | Public, read-only | Aggregate participants and accepted work by coin; no addresses or tokens. |
| POST /admin/participants | Admin token | Enrol a participant and return its one-time participantToken and stratumPassword. |
| POST /admin/payout-mode | Admin token | Select automatic or manual; default is manual. |
| GET /admin/participants | Admin token | List participant identities and payout addresses. |
| GET /admin/blocks | Admin token | Inspect prepared allocations. |
| GET /me | Participant token | Read the participant's own shares, accepted work and pending reward atoms. |
| POST /internal/authorize | Backend token only | Verify a participant ID and its Stratum password. |
| POST /internal/shares | Backend token only | Append a share already accepted by CKPool. |
| POST /internal/candidates | Backend token only | Record a found block and freeze the accepted-share sequence used for PPLNS. |
| GET /internal/candidates | Backend token only | Retrieve candidates still awaiting maturity checks. |
| POST /internal/candidates/orphan | Backend token only | Mark a candidate orphaned after a canonical-chain mismatch. |
| POST /internal/blocks | Backend token only | Prepare an allocation only when the trusted coin backend reports mature=true and a saved candidate exists. |

Participant enrolment requires the administrator token. There is no public self-enrolment. The participant token and Stratum password are shown only once. The server stores hashes, not plaintext copies.

The internal share endpoint must only be called by the trusted adapter after CKPool has accepted the share and written a result=true entry. The share sequence is allocated by the server; clients cannot choose it. A repeated (coin, shareId) is rejected.

For PPLNS correctness, the watcher records a candidate when it first detects the pool-paid block. The coordinator freezes the latest accepted-share sequence in the same journal transaction. When the block later matures, only shares at or before that cutoff can contribute to its allocation; work submitted during the 100-confirmation period is excluded. The candidate itself is durable, so restarting the watcher does not move the cutoff forward.

The BTC watcher is a trusted service, not an independent proof system. It queries the configured local Bitcoin Core RPC, checks the active-chain hash, decoded coinbase, payout address and confirmation count. Its RPC credentials and backend token are stored in a private local config. A reorganisation before maturity orphans the candidate and triggers a rescan. A mature allocation remains awaiting_settlement; no transaction is signed or broadcast.

## BTC Stratum V1 backend installation

The production installer is separate from the coordinator installer. It must be run only after the coordinator is installed and the existing server CKPool config/binary are present.

As the bdl account:

    cd /home/bdl/bdl-pool-site
    bash api/collective-btc-install.sh

The installer asks for:
1. One common BTC mainnet reward address. This is the address used by the new CKPool pool-mode instance for found block rewards. It is not a private key or wallet seed.
2. A PPLNS window, expressed in work units. 1,000,000 work units equals share difficulty 1. The operator must select the window; the installer does not guess one.

The address, RPC credentials and tokens are stored only in local private files; never commit them to Git or send them in chat.

Before installing systemd units, the script:

1. Reads /home/bdl/mining/ckpool/local.conf and preserves its local Bitcoin Core RPC settings.
2. Creates a distinct collective.conf with serverurl 127.0.0.1:3338, a unique mining IPC socket, a separate share log, donation set to zero and initial vardiff difficulty 1.
3. Generates private configs for the share collector and Bitcoin Core watcher.
4. Contacts only the configured local Bitcoin Core RPC to read the current chain height. The watcher starts at the next height, so blocks found by the existing solo pool before installation are not treated as collective rewards.
5. Runs ckpool -T against the new config and checks that ports 3337 and 3338 are free.
6. Installs and starts only the new CKPool collective, Stratum ingest and BTC block-watcher units.

The existing solo CKPool service on 3333/3336, Bitcoin Core, BCHN/CashStratum, public API and site are not restarted or reconfigured.

After the local services are active, test the dedicated OpenPort profile:

    python3 /home/bdl/bdl-pool-site/monitoring/try-openport.py test --profile collective

Open the test URL printed by the command from an external connection. After the tunnel test succeeds, install only that profile:

    bash /home/bdl/bdl-pool-site/monitoring/install-openport.sh collective

This does not restart the existing V1, V2 or API relay services. The public TCP host/port are saved locally in:

    /home/bdl/.local/state/bdl-pool-openport/collective/public.json

Use that reported host and port in BDLMinerV2 with Stratum V1. Set the username to participantId.worker and the password to the participant's stratumPassword. Do not use participantToken as the mining password.

The BTC pool-mode instance sends its block reward to the one common BTC address. The coordinator records work contributions and prepares a PPLNS allocation after the candidate reaches 100 confirmations. Distribution is still a manual/operator process; the software does not sign or broadcast payouts. Keep the service in controlled testing until settlement and firmware opt-in are completed.

## Payout modes and settlement

| Mode | Intended behavior |
| --- | --- |
| manual | Prepare allocations for operator review. No transaction is signed automatically. |
| automatic | Future settlement worker may queue transfers to each participant's validated address, subject to confirmations, minimums and fees. |

The default is manual. Neither mode broadcasts payments. Automatic payments require an isolated signer/wallet adapter, spending limits, an auditable payment journal, per-chain transaction construction, address/script checks and recovery tests.

## Remaining implementation stages

1. **Manual settlement.** Export/approve per-participant PPLNS allocations and perform the first controlled manual distribution. Add treasury reconciliation, audit records, fee accounting and recovery checks.
2. **Firmware integration.** Add a persistent, explicit collective-mining ON/OFF setting, separate from anonymous statistics sharing. OFF must leave the existing individual pool configuration untouched. ON must only be offered when the selected coin backend reports ready.
3. **Dashboard.** Show mode, participants, accepted work, estimated contribution, mature blocks, pending allocations and completed transactions without exposing other participants' payout addresses.
4. **Other coin adapters.** Add BCH, BSV, XEC, DGB and PPC only with their own tested node RPC, templates, Stratum backends, share validation and maturity rules.
5. **Automatic settlement.** Implement only after wallet isolation, per-chain transaction construction, address/script checks, fee/minimum limits, replay protection and recovery tests.

## Safety boundary

This backend is opt-in and separate from the existing solo services. The BTC Stratum V1 proxy, pool-mode CKPool instance and block watcher use separate ports, configs and service units. No production service is installed by this repository change; installation requires an explicit command on the pool server. No payment is signed or broadcast. The PR remains a draft, and collectiveMiningReady deliberately remains false until the remaining firmware, settlement and operator-validation stages are complete.
