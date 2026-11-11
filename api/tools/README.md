# BDL Pool tools

These tools consume the public, read-only BDL Pool API. They do not connect to
Stratum, control miners, or submit shares. The website already includes a pool
dashboard and online-miners view; these tools add CLI monitoring, an on-demand
Discord status command, and Prometheus metrics.

Set `BDL_POOL_API_URL` to override the API base URL. The default is
`https://api.bdl-pool.com:36018/api/v1`.

## Rust pool monitor

Requires Rust and Cargo. It prints readiness, hashrate, workers, blocks, best
share, Stratum check, and snapshot age. Readiness is based on the latest Stratum
V1 check; stale or syncing snapshots are identified separately.

```bash
cd api/tools/pool-monitor
cargo run -- --once
cargo run
```

The continuous monitor polls every 60 seconds by default. Set
`BDL_POOL_POLL_SECONDS` to change the interval; values below five seconds are
rejected.

## Discord bot

Requires Node.js 22.18 or newer and a Discord application configured with the
`bot` and `applications.commands` install scopes. The bot uses only the Guilds
intent and answers `/pool` with an ephemeral status and statistics response.
Create an application in the Discord developer portal and invite it to the
server with those scopes before starting it.

```bash
cd api/tools/discord-bot
npm install
npm run build
```

Set `DISCORD_TOKEN` and `DISCORD_APPLICATION_ID` in the process environment. Set
`DISCORD_GUILD_ID` for immediate registration in one test server; omit it to
register the global command. Register the command, then start the bot:

```bash
npm run deploy-commands
npm start
```

Never commit the bot token or put it in source control. Rotate it in the Discord
developer portal if it is exposed.

## Prometheus exporter

Requires Go 1.21 or newer. The exporter polls the API every 30 seconds and
exposes `/metrics` and `/healthz`. It has no third-party Go dependencies.

```bash
cd api/tools/prometheus-exporter
go test ./...
go run .
```

Prometheus metrics listen on `127.0.0.1:9108` by default. Set
`PROMETHEUS_LISTEN_ADDRESS` to change the bind address and
`BDL_POOL_SCRAPE_SECONDS` to change the poll interval (five seconds to one day).
A failed API scrape sets `bdl_pool_api_up` to `0`, marks the snapshot stale, and
clears the readiness metric while retaining last-known numeric measurements.
No address, worker name, or other miner identifier is exported.

Example Prometheus scrape configuration:

```yaml
scrape_configs:
  - job_name: bdl-pool
    static_configs:
      - targets: ["127.0.0.1:9108"]
```
