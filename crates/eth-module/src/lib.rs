//! # eth-module
//!
//! SpacetimeDB module integration for Ethereum.
//!
//! ## What lives here
//! - SpacetimeDB table definitions for Ethereum data
//! - `EthScheduler` — drives polling from a `@schedule` reducer
//! - `#[eth_event]` proc-macro (generates topic hash + typed decoder)
//! - `EthReducer` trait — implement to handle decoded events

// Modules will be uncommented as we implement them:
// pub mod tables;
// pub mod scheduler;
// pub mod traits;
