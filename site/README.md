# BDL Pool in Rust

The typed native generator in site/src/ owns twelve French/English pages,
metadata, navigation and the public export. The browser application in
site/browser/src/ compiles to WebAssembly: network selection, CashAddr checksum
validation, Stratum V1/V2 settings, monitoring caches, miner lookup, pagination,
themes, menus and Canvas charts are Rust. No React, Next.js or handwritten
JavaScript is loaded. HTML/CSS remain the document/presentation formats;
wasm-bindgen generates the JavaScript required to call browser APIs.

Public settings live in config.json and are compiled into the WASM application.
Changing them requires a build. The build fingerprints the generated loader,
JavaScript bindings, WASM and styles for cache invalidation. Browser assets are
built locally; production serves the same routes on GitHub Pages.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.105 --locked
cargo run --locked --release --manifest-path site/Cargo.toml
python3 -m http.server 3000 --directory out --bind 127.0.0.1
```

Python's static preview server is a development convenience. It is not part of
production. Node/TypeScript are used for independent browser tests only.

```sh
cargo fmt --manifest-path site/browser/Cargo.toml -- --check
cargo clippy --locked --manifest-path site/browser/Cargo.toml --target wasm32-unknown-unknown -- -D warnings
cargo test --locked --manifest-path site/browser/Cargo.toml
cargo test --locked --manifest-path site/Cargo.toml
npm ci
npm run typecheck
npm run test:e2e
```

Native API, filtered connection collection and public snapshot collection are
in services/. api/install.sh compiles and installs the Rust services using the
existing public configuration, certificates and ports. Changing source does
not restart the pool server: run that installer on the server to activate it.
Bitcoin Core, CKPool, mining firmware and the OpenPort transport are separate
programs. Historical operational migration tools and test fixtures are not
part of the Rust application runtime.
