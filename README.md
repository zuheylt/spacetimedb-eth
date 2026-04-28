# spacetimedb-ethereum

Ethereum/Web3 support for [SpacetimeDB](https://spacetimedb.com), built from primitives.

[![CI](https://github.com/zuheylt/spacetimedb-eth/actions/workflows/ci.yml/badge.svg)](https://github.com/zuheylt/spacetimedb-eth/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/eth-core.svg)](https://crates.io/crates/eth-core)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[![MSRV: 1.75](https://img.shields.io/badge/MSRV-1.75-blue)](https://blog.rust-lang.org/2023/12/28/Rust-1.75.0.html)

> **Status:** Active development — `0.1.x` is a work in progress. The API is not yet stable.

---

## What this is

A Cargo workspace of three crates that let any SpacetimeDB server module interact with an EVM-compatible blockchain. No `ethers-rs`, no `alloy` — every primitive is implemented from spec so the core layer compiles to `wasm32-unknown-unknown` without modification.

The design goal is a clean separation between pure cryptographic logic (which must run inside a SpacetimeDB WebAssembly module) and async network I/O (which runs in a sidecar process alongside it).

---

## Crates

| Crate | Description | `no_std` | Status |
|-------|-------------|----------|--------|
| [`eth-core`](crates/eth-core) | Keccak-256, RLP, ABI codec, EIP-155 transactions, secp256k1 signing | ✅ | In progress |
| [`eth-module`](crates/eth-module) | SpacetimeDB table types, scheduler, `#[eth_event]` macro | ❌ | Planned |
| [`eth-sidecar`](crates/eth-sidecar) | Async Tokio binary — JSON-RPC bridge to a SpacetimeDB instance | ❌ | Planned |

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                 SpacetimeDB Module                    │
│  ┌─────────────────┐   ┌────────────────────────┐    │
│  │   eth-module    │──▶│  Tables:               │    │
│  │  EthScheduler   │   │  pending_tx            │    │
│  │  #[eth_event]   │   │  confirmed_tx          │    │
│  └────────┬────────┘   │  event_log             │    │
│           │            │  watched_contracts     │    │
│  ┌────────▼────────┐   └────────────────────────┘    │
│  │    eth-core     │  no_std · wasm32-safe            │
│  │  Keccak-256     │  RLP · ABI · EIP-155 · k256     │
│  └─────────────────┘                                  │
└───────────────────────────┬──────────────────────────┘
                            │ reads / writes tables
               ┌────────────▼────────────┐
               │      eth-sidecar        │  standalone binary
               │  eth_getLogs polling    │  Tokio · async
               │  tx signing + broadcast │
               │  /health endpoint       │
               └────────────┬────────────┘
                            │ JSON-RPC
               ┌────────────▼────────────┐
               │    Ethereum Node        │
               │   (any EVM chain)       │
               └─────────────────────────┘
```

---

## Using `eth-core`

Add to your `Cargo.toml`:

```toml
[dependencies]
eth-core = "0.1"
```

For `no_std` / WebAssembly targets:

```toml
[dependencies]
eth-core = { version = "0.1", default-features = false }
```

### Feature flags

| Flag | Default | Adds |
|------|---------|------|
| `std` | yes | `std::error::Error` impl on `EthError` |
| `serde` | no | `Serialize` / `Deserialize` on all public types |

### Keccak-256

```rust
use eth_core::keccak::keccak256;

// Hash a function signature to get its 4-byte selector
let hash = keccak256(b"transfer(address,uint256)");
let selector = &hash[..4]; // first 4 bytes

// Hash an event signature to get its topic-0
let topic = keccak256(b"Transfer(address,address,uint256)");
assert_eq!(
    topic,
    hex_literal::hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
);
```

---

## Building and testing

```bash
# Run all tests
cargo test --all-features

# Verify the core crate compiles with no_std
cargo test -p eth-core --no-default-features

# Verify it cross-compiles to WebAssembly
cargo build -p eth-core --no-default-features --target wasm32-unknown-unknown

# Lint
cargo clippy --all-features -- -D warnings
```

Rust 1.75+ required. Install via [rustup](https://rustup.rs).

---

## What's implemented

- [x] Keccak-256 — full sponge construction from spec, verified against published Ethereum test vectors and cross-checked against the `sha3` crate
- [x] RLP encoding and decoding
- [x] `Address`, `TxHash`, `Bytes32` newtypes with hex `Display` / `FromStr`
- [ ] ABI encoding and decoding (all Solidity types)
- [ ] EIP-155 transaction builder + secp256k1 signing
- [ ] JSON-RPC client (`eth_getLogs`, `eth_sendRawTransaction`, `eth_getTransactionReceipt`)
- [ ] SpacetimeDB table definitions and scheduler
- [ ] `#[eth_event]` proc macro

---

## Why not ethers-rs / alloy?

Both are excellent for applications that run on a standard server. This library exists for a different constraint: SpacetimeDB modules compile to WebAssembly and run in a sandboxed environment with no network access and no standard library. Neither `ethers-rs` nor `alloy` supports `no_std`, so they cannot be imported inside a module. Building the primitives from scratch also means the dependency tree stays small and auditable.

---

## Contributing

Issues and pull requests welcome. Please run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test --all-features` before submitting.

---

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
