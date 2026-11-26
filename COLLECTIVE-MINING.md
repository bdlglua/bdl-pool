# BDL Collective Mining

**Status: accounting coordinator and first BTC Stratum V1 adapter are implemented in code. The adapter has not been installed on the production pool server. The system is not yet a complete reward-sharing service.**

This project adds a self-hosted collective-mining backend for the SHA-256d coin profiles exposed by BDLMinerV2. It is separate from the existing anonymous hashrate/statistics sharing. The existing public statistics API remains read-only.

## Supported coin profiles

The accounting engine recognises all six current BDLMinerV2 SHA-256d profiles:

- BTC — Bitcoin
- BCH — Bitcoin Cash
- BSV — Bitcoin SV
- XEC — eCash
- DGB — DigiByte SHA-256d
- PPC — Peercoin proof-of-work

The first live-backend implementation targets BTC over Stratum V1. Other coin-specific Stratum/node adapters are not implemented yet.

## Implemented components

- Deterministic PPLNS accounting. Each accepted share is weighted using its assigned share difficulty. Largest-remainder rounding allocates the complete block reward exactly in the coin's smallest unit.
- A private coordinator bound to 127.0.0.1:8082, with a durable append-only journal and single-process file lock.
- Separate administrator, backend, participant-dashboard and Stratum-mining credentials.
- A BTC Stratum V1 authentication proxy intended for 127.0.0.1:3337. It checks the participant ID and its dedicated Stratum password with the coordinator before forwarding mining.authorize.
- A separate pool-mode CKPool instance intended for 127.0.0.1:3338. It uses the existing local Bitcoin Core RPC configuration but has its own common BTC payout address, listener, mining IPC socket, runtime directory and share-log directory.
- A CKPool share-log collector. It reads only CKPool records with result=true, converts the assigned diff field to work units, and submits accepted work using the server-only backend token. A persistent cursor prevents normal restarts from re-reading the complete journal; duplicate IDs are rejected by the coordinator.
- A dedicated OpenPort profile named collective, forwarded to local port 3337. It can be installed without restarting the existing V1, V2 or HTTPS relays.

The coordinator's public status still reports collectiveMiningReady=false. This is intentional: block detection, node confirmation/maturity handling and reward settlement are not yet connected. The first BTC adapter code is present, but no production service has been installed and no real miners have been switched to it.

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
| POST /internal/authorize | Backend token only | Verify a participant ID and its Stratum password; does not expose the password hash. |
| POST /internal/shares | Backend token only | Append a share already accepted by CKPool. |
| POST /internal/blocks | Backend token only | Prepare an allocation only when the trusted coin backend reports mature=true. |

Participant enrolment requires the administrator token. There is no public self-enrolment. The participant token and Stratum password are shown only once. The server stores hashes, not plaintext copies.

The internal share endpoint is not a simulated mining endpoint. It must only be called by the trusted adapter after CKPool has accepted the share and written a result=true entry. The share sequence is allocated by the server; clients cannot choose it. A repeated (coin, shareId) is rejected.

The block endpoint must only be called by a trusted backend after the coin node confirms maturity. The same (coin, blockId) cannot be recorded twice with different reward data. The resulting allocation remains awaiting_settlement and paymentsEnabled=false.

## BTC Stratum V1 backend installation

The production installer is separate from the coordinator installer. It must be run only after the coordinator is installed and the existing server CKPool config/binary are present.

As the bdl account:

    cd /home/bdl/bdl-pool-site
    bash api/collective-btc-install.sh

The installer asks for one common BTC mainnet reward address. This is the address used by the new CKPool pool-mode instance for found block rewards. It is not a private key or wallet seed. The address and RPC config are stored only in local private files; never commit them to Git or send them in chat.

Before installing systemd units, the script:

1. Reads the existing /home/bdl/mining/ckpool/local.conf and preserves its btcd RPC/ZMQ connection settings.
2. Creates a distinct collective.conf with serverurl 127.0.0.1:3338, a unique mining IPC socket, a separate share log, donation set to zero and initial vardiff difficulty 1.
3. Generates a private ingest config holding the backend token.
4. Runs ckpool -T against the new config.
5. Checks that ports 3337 and 3338 are free.
6. Installs and starts only the new CKPool collective and Stratum ingest units.

The existing solo CKPool service on 3333/3336, Bitcoin Core, BCHN/CashStratum, public API and site are not restarted or reconfigured.

After the local services are active, test the dedicated OpenPort profile:

    python3 /home/bdl/bdl-pool-site/monitoring/try-openport.py test --profile collective

Open the test URL printed by the command from an external connection. After the tunnel test succeeds, install only that profile:

    bash /home/bdl/bdl-pool-site/monitoring/install-openport.sh collective

This does not restart the existing V1, V2 or API relay services. The public TCP host/port are saved locally in:

    /home/bdl/.local/state/bdl-pool-openport/collective/public.json

Use that reported host and port in BDLMinerV2 with Stratum V1. Set the username to participantId.worker and the password to the participant's stratumPassword. Do not use participantToken as the mining password.

The BTC pool-mode instance sends any block reward to the one common BTC address. The coordinator records accepted contributions, but the node/block watcher and settlement worker are not yet implemented. Therefore this stage does not automatically divide BTC or send participant payouts. Keep collective mining in a controlled test until block handling, maturity and manual settlement are completed.

## Payout modes and settlement

| Mode | Intended behavior |
| --- | --- |
| manual | Prepare allocations for operator review. No transaction is signed automatically. |
| automatic | Future settlement worker may queue transfers to each participant's validated address, subject to confirmations, minimums and fees. |

The default is manual. At present, neither mode broadcasts payments. Automatic payments require an isolated signer/wallet adapter, spending limits, an auditable payment journal, per-chain transaction construction and recovery tests.

## Remaining implementation stages

1. **BTC node/block integration.** Detect block candidates from Bitcoin Core, verify the block and spendable subsidy/fees, track the required maturity/confirmations, and call the coordinator once per mature block.
2. **Manual settlement.** Export/approve per-participant PPLNS allocations and perform the first controlled manual distribution. Add audit records and recovery checks.
3. **Other coin adapters.** Add BCH, BSV, XEC, DGB and PPC only with their own tested node RPC, templates, Stratum backends, share validation and maturity rules.
4. **Firmware integration.** Add a persistent, explicit collective-mining ON/OFF setting, separate from anonymous statistics sharing. OFF must leave the existing individual pool configuration untouched. ON must only be offered when the selected coin backend reports ready.
5. **Dashboard.** Show mode, participants, accepted work, estimated contribution, mature blocks, pending allocations and completed transactions without exposing other participants' payout addresses.
6. **Automatic settlement.** Only after wallet isolation, per-chain transaction construction, address/script checks, fee/minimum limits, replay protection and recovery tests.

## Safety boundary

This backend is opt-in and separate from the existing solo services. The BTC Stratum V1 proxy and pool-mode CKPool instance use separate ports and files. No service is installed by this repository change; installation requires an explicit command on the pool server. No payment is signed or broadcast by this milestone. A hashrate dashboard alone is not collective mining, and collectiveMiningReady remains false until the missing node, block and settlement components are in place.
