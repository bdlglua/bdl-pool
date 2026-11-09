# API clients

Runnable read-only health-check clients for the BDL Pool API are provided in
`typescript/`, `rust/`, and `go/`.

The API base URL can be overridden with `BDL_POOL_API_URL` (without a trailing
slash). Each client calls `GET /health`, checks the HTTP status, and validates
the API response.

## TypeScript

Requires Node.js 22.18 or newer. Run the TypeScript source using Node's built-in
type stripping:

```bash
cd api/examples/typescript
npm start
```

To type-check it, install the development dependency and run:

```bash
npm install
npm run check
```

## Rust

Requires Rust and Cargo. Dependencies use the Rustls TLS backend:

```bash
cd api/examples/rust
cargo run
```

## Go

Requires Go 1.21 or newer. The client uses only the standard library:

```bash
cd api/examples/go
go run .
```
