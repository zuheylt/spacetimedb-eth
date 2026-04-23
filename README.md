# spacetimedb-ethereum

> Ethereum/Web3 support for [SpacetimeDB](https://spacetimedb.com) — built from primitives.

[![CI](https://github.com/zuheylt/spacetimedb-eth/actions/workflows/ci.yml/badge.svg)](https://github.com/zuheylt/spacetimedb-eth/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/eth-core.svg)](https://crates.io/crates/eth-core)
[![MSRV: 1.75](https://img.shields.io/badge/MSRV-1.75-blue)](https://blog.rust-lang.org/2023/12/28/Rust-1.75.0.html)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

> **Repository:** github.com/zuheylt/spacetimedb-eth

## What is this?

A Cargo workspace of three crates that let you interact with any EVM-compatible
blockchain from within a SpacetimeDB server module — without pulling in ethers-rs
or alloy. Everything is built from primitives so you can understand exactly what
is happening on-chain.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                SpacetimeDB Module                    │
│  ┌──────────────┐   ┌──────────────────────────┐    │
│  │  eth-module  │──▶│  SpacetimeDB tables:     │    │
│  │  EthScheduler│   │  pending_tx, event_log,  │    │
│  │  #[eth_event]│   │  confirmed_tx, contracts  │    │
│  └──────┬───────┘   └──────────────────────────┘    │
│         │ depends on                                  │
│  ┌──────▼───────┐                                    │
│  │   eth-core   │  no_std · wasm32-safe              │
│  │  Keccak-256  │  RLP · ABI · EIP-155 tx · k256    │
│  └──────────────┘                                    │
└──────────────────────────────┬──────────────────────┘
                               │ reads/writes tables
                    ┌──────────▼──────────┐
                    │    eth-sidecar      │  standalone binary
                    │  JSON-RPC polling   │  tokio · async
                    │  tx broadcasting    │
                    │  health endpoint    │
                    └──────────┬──────────┘
                               │ JSON-RPC
                    ┌──────────▼──────────┐
                    │  Ethereum Node      │
                    │  (any EVM chain)    │
                    └─────────────────────┘
```

## Crates

| Crate | Description | `no_std` | Feature flags |
|-------|-------------|----------|---------------|
| [`eth-core`](crates/eth-core) | Pure Rust Ethereum primitives | Yes | `std` (default), `serde` |
| [`eth-module`](crates/eth-module) | SpacetimeDB module integration | No | `std` (default), `serde` |
| [`eth-sidecar`](crates/eth-sidecar) | Async RPC bridge binary | No (binary) | `serde` |

## Quick Start

_Coming in step 11 — examples will be filled in as the library is built._

## MSRV

Minimum Supported Rust Version: **1.75** (stable, December 2023).

## License

Licensed under either of:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
