//! # eth-core
//!
//! Pure-Rust, `no_std`-compatible Ethereum primitives.
//!
//! Compiles to `wasm32-unknown-unknown` with `--no-default-features`.
//!
//! ## Feature flags
//!
//! | Flag    | Default | What it adds |
//! |---------|---------|--------------|
//! | `std`   | yes     | `std::error::Error` impl on `EthError` via thiserror |
//! | `serde` | no      | `Serialize`/`Deserialize` on all public types |
//!
//! ## Modules (filled in as we build)
//! - `keccak`  — Keccak-256 hash
//! - `rlp`     — RLP encode / decode
//! - `types`   — `Address`, `TxHash`, `Bytes32` newtypes
//! - `abi`     — ABI encode / decode
//! - `tx`      — EIP-155 transaction builder + signing

#![cfg_attr(not(feature = "std"), no_std)]

// When no_std, we still need heap allocation (Vec, String, Box).
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod error;
pub mod keccak;
pub mod rlp;
// pub mod types;
// pub mod abi;
// pub mod tx;
