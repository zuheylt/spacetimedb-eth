//! # eth-sidecar
//!
//! Standalone async binary. Bridges Ethereum JSON-RPC ↔ SpacetimeDB.
//!
//! ## Responsibilities
//! - Poll `eth_getLogs` for watched contracts → write events to SpacetimeDB
//! - Read pending TX table → sign and broadcast via `eth_sendRawTransaction`
//! - Health/metrics HTTP endpoint (plain Tokio, no framework)
//! - Config: env vars + optional TOML file
//! - Graceful shutdown on SIGTERM / SIGINT

fn main() {
    // Implementation starts at step 8.
    println!("eth-sidecar stub");
}
