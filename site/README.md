# BDL Pool website in Rust

`site/src/main.rs` renders twelve French/English pages from shared HTML templates
and exports public assets to `out/`. The generator uses the Rust standard library
only. There is no React, Next.js, hydration bundle or Node server in the website.
GitHub Pages continues to serve `bdl-pool.com` with the same URLs and HTTPS.

```sh
cargo test --locked --manifest-path site/Cargo.toml
cargo run --locked --release --manifest-path site/Cargo.toml
python3 -m http.server 3000 --directory out --bind 127.0.0.1
```

Edit `site/templates/pages/` for translated content, `layout-fr.html` and
`layout-en.html` for shared chrome, and `document.html` for the document frame.
The typed Rust catalog owns routes, navigation, titles, descriptions,
canonical/alternate URLs, robots and sitemap. Text metadata is HTML-escaped;
unknown bindings fail the build. CSS/JS cache keys follow file contents.

## Browser features

Existing browser scripts in `assets/` retain monitoring, BTC/BCH selection,
CashAddr validation, Stratum V1/V2 settings, miner lookup, charts, menu navigation
and language links. `assets/theme.js` replaces the React theme component and
preserves its storage key, labels, theme-color values and theme-change event.

Scripts are ordinary deferred scripts in dependency order: configuration before
network selection, Chart.js before its adapter, adapters before consumers.
Body context is rendered directly by Rust; tools do not wait for hydration.
Translated content remains readable without JavaScript.

The Python API, Bitcoin/CKPool services, BCH relay and monitoring are independent
services and are unchanged. Only `assets/`, `config.js`, `data/`, CNAME,
`.nojekyll` and `api/openapi.json` are copied to the public export. Server and
monitoring source code is excluded.

## Validate and publish

```sh
cargo fmt --manifest-path site/Cargo.toml -- --check
cargo clippy --locked --manifest-path site/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path site/Cargo.toml
cargo run --locked --release --manifest-path site/Cargo.toml
npm ci
npm run typecheck
npm run test:e2e
```

Node/TypeScript are retained for Playwright tests. `npm run build` is a convenience
alias for Rust. The `Rust website` workflow checks Rust, builds the export, runs
browser tests, then publishes `out/` from `main`. Legacy branch-publication HTML
and Next.js sources are retired. No DNS, Stratum, API or access setting changes.
