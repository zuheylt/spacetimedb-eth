//! ABI encoding — converts Rust values to Ethereum ABI-encoded bytes.
//!
//! The ABI (Application Binary Interface) is how Ethereum encodes function
//! call arguments into the raw bytes that go in a transaction's `data` field.
//! Every call starts with a 4-byte selector followed by ABI-encoded arguments.
//!
//! # Example
//!
//! ```
//! use eth_core::abi::{AbiToken, encode, selector};
//!
//! // Build calldata for: transfer(address _to, uint256 _amount)
//! let recipient = [0xd8u8; 20];
//! let amount    = AbiToken::from_u64(1_000_000);
//!
//! let mut calldata = selector("transfer(address,uint256)").to_vec();
//! calldata.extend(encode(&[AbiToken::Address(recipient), amount]));
//!
//! assert_eq!(calldata.len(), 4 + 64); // 4-byte selector + two 32-byte words
//! assert_eq!(&calldata[..4], &[0xa9, 0x05, 0x9c, 0xbb]);
//! ```

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::{error::EthError, keccak::keccak256};

// ── Token type ───────────────────────────────────────────────────────────────

/// One ABI-encodable value — mirrors the types in a Solidity function signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiToken {
    /// `uint8` through `uint256` — 32 bytes big-endian, zero-padded on the left.
    Uint([u8; 32]),
    /// `address` — 20 bytes, zero-padded to 32 on the left.
    Address([u8; 20]),
    /// `bool`.
    Bool(bool),
    /// `bytes1` through `bytes32` — validated to 1–32 bytes; zero-padded on the right when encoded.
    FixedBytes(Vec<u8>),
    /// `bytes` — dynamic byte array.
    Bytes(Vec<u8>),
    /// `string` — dynamic UTF-8 string (encoded identically to `Bytes`).
    Str(Vec<u8>),
    /// Tuple / function argument list — encodes as a nested head-tail sequence.
    Tuple(Vec<AbiToken>),
}

impl AbiToken {
    /// Constructs `uint256` from a `u64`.
    ///
    /// ```
    /// use eth_core::abi::AbiToken;
    /// let t = AbiToken::from_u64(1);
    /// assert_eq!(t, AbiToken::Uint({ let mut b = [0u8; 32]; b[31] = 1; b }));
    /// ```
    pub fn from_u64(n: u64) -> Self {
        let mut bytes = [0u8; 32];
        bytes[24..].copy_from_slice(&n.to_be_bytes());
        AbiToken::Uint(bytes)
    }

    /// Constructs `uint256` from a `u128`.
    pub fn from_u128(n: u128) -> Self {
        let mut bytes = [0u8; 32];
        bytes[16..].copy_from_slice(&n.to_be_bytes());
        AbiToken::Uint(bytes)
    }

    /// Constructs a `bytes<N>` token. Returns an error if `data` is empty or longer than 32 bytes.
    ///
    /// ```
    /// use eth_core::abi::AbiToken;
    /// let t = AbiToken::fixed_bytes(b"abc").unwrap();
    /// assert!(matches!(t, AbiToken::FixedBytes(_)));
    /// ```
    pub fn fixed_bytes(data: &[u8]) -> Result<Self, EthError> {
        if data.is_empty() || data.len() > 32 {
            return Err(EthError::AbiInvalidBytesLength(data.len().min(255) as u8));
        }
        Ok(AbiToken::FixedBytes(data.to_vec()))
    }
}

// ── Encoding internals ────────────────────────────────────────────────────────

/// True when a token's head slot must hold an offset rather than an inline value.
fn is_dynamic(token: &AbiToken) -> bool {
    match token {
        AbiToken::Bytes(_) | AbiToken::Str(_) => true,
        AbiToken::Tuple(tokens) => tokens.iter().any(is_dynamic),
        _ => false,
    }
}

/// Bytes a token occupies in the head section.
/// Dynamic → 32 (the offset word). Static tuple → sum of members. Everything else → 32.
fn head_size(token: &AbiToken) -> usize {
    if is_dynamic(token) {
        return 32;
    }
    match token {
        AbiToken::Tuple(tokens) => tokens.iter().map(head_size).sum(),
        _ => 32,
    }
}

/// Encodes `n` as a 32-byte big-endian word (used for offsets and byte-lengths).
fn word(n: usize) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&(n as u64).to_be_bytes());
    out
}

/// Appends `data` to `buf`, then zero-pads to the next 32-byte boundary.
fn write_padded(buf: &mut Vec<u8>, data: &[u8]) {
    buf.extend_from_slice(data);
    let rem = data.len() % 32;
    if rem != 0 {
        buf.extend(core::iter::repeat(0u8).take(32 - rem));
    }
}

/// Writes the inline (static) encoding of `token` into `out`.
fn write_static(token: &AbiToken, out: &mut Vec<u8>) {
    match token {
        AbiToken::Uint(bytes) => out.extend_from_slice(bytes),
        AbiToken::Address(addr) => {
            out.extend_from_slice(&[0u8; 12]);
            out.extend_from_slice(addr);
        }
        AbiToken::Bool(b) => {
            out.extend_from_slice(&[0u8; 31]);
            out.push(*b as u8);
        }
        AbiToken::FixedBytes(data) => {
            out.extend_from_slice(data);
            out.extend(core::iter::repeat(0u8).take(32 - data.len()));
        }
        AbiToken::Tuple(tokens) => out.extend(encode(tokens)),
        _ => unreachable!(),
    }
}

/// Produces the tail encoding of a dynamic token.
fn encode_tail(token: &AbiToken) -> Vec<u8> {
    match token {
        AbiToken::Bytes(data) | AbiToken::Str(data) => {
            let mut out = Vec::new();
            out.extend_from_slice(&word(data.len()));
            write_padded(&mut out, data);
            out
        }
        AbiToken::Tuple(tokens) => encode(tokens),
        _ => unreachable!(),
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// ABI-encodes a sequence of tokens — the argument list of a function call.
///
/// The output is the raw bytes that follow the 4-byte selector in a transaction's
/// `data` field. Combine with [`selector`] to build complete calldata.
///
/// ```
/// use eth_core::abi::{AbiToken, encode};
///
/// let out = encode(&[AbiToken::from_u64(1), AbiToken::from_u64(2)]);
/// assert_eq!(out.len(), 64); // two 32-byte words
/// assert_eq!(out[31], 1);
/// assert_eq!(out[63], 2);
/// ```
pub fn encode(tokens: &[AbiToken]) -> Vec<u8> {
    let total_head: usize = tokens.iter().map(head_size).sum();
    let mut head = Vec::with_capacity(total_head);
    let mut tail: Vec<u8> = Vec::new();

    for token in tokens {
        if is_dynamic(token) {
            head.extend_from_slice(&word(total_head + tail.len()));
            tail.extend(encode_tail(token));
        } else {
            write_static(token, &mut head);
        }
    }

    head.extend(tail);
    head
}

/// Returns the 4-byte function selector for a Solidity signature string.
///
/// The selector is the first 4 bytes of `keccak256(signature)`. It is
/// prepended to ABI-encoded arguments to identify which function to call.
///
/// ```
/// use eth_core::abi::selector;
///
/// assert_eq!(selector("transfer(address,uint256)"), [0xa9, 0x05, 0x9c, 0xbb]);
/// ```
pub fn selector(sig: &str) -> [u8; 4] {
    let hash = keccak256(sig.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(feature = "std"))]
    use alloc::vec;

    // ── selector ─────────────────────────────────────────────────────────────

    #[test]
    fn selector_transfer() {
        assert_eq!(selector("transfer(address,uint256)"), [0xa9, 0x05, 0x9c, 0xbb]);
    }

    #[test]
    fn selector_balance_of() {
        assert_eq!(selector("balanceOf(address)"), [0x70, 0xa0, 0x82, 0x31]);
    }

    // ── static types ─────────────────────────────────────────────────────────

    #[test]
    fn encode_uint_one() {
        let out = encode(&[AbiToken::from_u64(1)]);
        assert_eq!(out.len(), 32);
        assert_eq!(out[31], 1);
        assert!(out[..31].iter().all(|&b| b == 0));
    }

    #[test]
    fn encode_uint_max_u64() {
        let out = encode(&[AbiToken::from_u64(u64::MAX)]);
        assert_eq!(&out[24..], &u64::MAX.to_be_bytes());
        assert!(out[..24].iter().all(|&b| b == 0));
    }

    #[test]
    fn encode_uint_from_u128() {
        let n: u128 = 0xdeadbeef_cafebabe_u128;
        let out = encode(&[AbiToken::from_u128(n)]);
        assert_eq!(&out[16..], &n.to_be_bytes());
        assert!(out[..16].iter().all(|&b| b == 0));
    }

    #[test]
    fn encode_address() {
        let addr = [0xabu8; 20];
        let out = encode(&[AbiToken::Address(addr)]);
        assert_eq!(out.len(), 32);
        assert!(out[..12].iter().all(|&b| b == 0)); // zero-padded on left
        assert_eq!(&out[12..], &addr);
    }

    #[test]
    fn encode_bool_true() {
        let out = encode(&[AbiToken::Bool(true)]);
        assert_eq!(out.len(), 32);
        assert_eq!(out[31], 1);
        assert!(out[..31].iter().all(|&b| b == 0));
    }

    #[test]
    fn encode_bool_false() {
        let out = encode(&[AbiToken::Bool(false)]);
        assert!(out.iter().all(|&b| b == 0));
    }

    #[test]
    fn encode_fixed_bytes_3() {
        let out = encode(&[AbiToken::fixed_bytes(b"abc").unwrap()]);
        assert_eq!(out.len(), 32);
        assert_eq!(&out[..3], b"abc");               // left-aligned
        assert!(out[3..].iter().all(|&b| b == 0));    // right-padded
    }

    #[test]
    fn encode_fixed_bytes_32() {
        let data = [0xffu8; 32];
        let out = encode(&[AbiToken::fixed_bytes(&data).unwrap()]);
        assert_eq!(out, data);
    }

    // ── dynamic types ────────────────────────────────────────────────────────

    #[test]
    fn encode_bytes_hello() {
        // Single `bytes` argument: offset=32, length=5, data="hello" padded to 32.
        let out = encode(&[AbiToken::Bytes(b"hello".to_vec())]);
        assert_eq!(out.len(), 96);  // 32 offset + 32 length + 32 data
        assert_eq!(out[31], 32);    // offset points past the head
        assert_eq!(out[63], 5);     // length = 5
        assert_eq!(&out[64..69], b"hello");
        assert!(out[69..].iter().all(|&b| b == 0));
    }

    #[test]
    fn encode_bytes_empty() {
        let out = encode(&[AbiToken::Bytes(vec![])]);
        assert_eq!(out.len(), 64); // offset + length(0), no data words
        assert_eq!(out[31], 32);   // offset
        assert_eq!(out[63], 0);    // length = 0
    }

    #[test]
    fn encode_bytes_exact_32() {
        // 32 bytes of data: no extra padding word needed.
        let data = vec![0xffu8; 32];
        let out = encode(&[AbiToken::Bytes(data)]);
        assert_eq!(out.len(), 96); // offset + length + 32 data bytes
    }

    #[test]
    fn encode_bytes_33() {
        // 33 bytes: data spills into a second word (padded to 64 bytes).
        let data = vec![0xabu8; 33];
        let out = encode(&[AbiToken::Bytes(data)]);
        assert_eq!(out.len(), 128); // offset + length + 64 data bytes
    }

    // ── mixed static + dynamic ────────────────────────────────────────────────

    #[test]
    fn encode_uint_then_bytes() {
        // encode(uint256(0x42), bytes("hi"))
        // head: [word(0x42), offset(64)]
        // tail: [word(2), "hi" + 30 zeros]
        let out = encode(&[AbiToken::from_u64(0x42), AbiToken::Bytes(b"hi".to_vec())]);
        assert_eq!(out.len(), 128);  // 64 head + 64 tail
        assert_eq!(out[31], 0x42);  // uint value
        assert_eq!(out[63], 64);    // offset = past both head words
        assert_eq!(out[95], 2);     // length of "hi"
        assert_eq!(&out[96..98], b"hi");
    }

    #[test]
    fn encode_two_dynamic() {
        // encode(bytes("ab"), bytes("cd"))
        // head: [offset(64), offset(64+64)]
        // tail: [word(2),"ab"+30zeros, word(2),"cd"+30zeros]
        let out = encode(&[
            AbiToken::Bytes(b"ab".to_vec()),
            AbiToken::Bytes(b"cd".to_vec()),
        ]);
        assert_eq!(out.len(), 192); // 64 head + 128 tail
        assert_eq!(out[31], 64);   // first offset
        assert_eq!(out[63], 128);  // second offset
    }

    // ── tuple ─────────────────────────────────────────────────────────────────

    #[test]
    fn encode_static_tuple() {
        // A static tuple encodes inline — no offset word.
        let out = encode(&[AbiToken::Tuple(vec![
            AbiToken::from_u64(1),
            AbiToken::from_u64(2),
        ])]);
        assert_eq!(out.len(), 64); // two 32-byte words, no indirection
        assert_eq!(out[31], 1);
        assert_eq!(out[63], 2);
    }

    // ── full calldata ─────────────────────────────────────────────────────────

    #[test]
    fn transfer_calldata() {
        // ERC-20 transfer(address _to, uint256 _amount)
        let recipient = [0x11u8; 20];
        let amount    = AbiToken::from_u64(500);
        let args      = encode(&[AbiToken::Address(recipient), amount]);

        let mut call = selector("transfer(address,uint256)").to_vec();
        call.extend(args);

        assert_eq!(call.len(), 4 + 64);
        assert_eq!(&call[..4], &[0xa9, 0x05, 0x9c, 0xbb]);
        // address is zero-padded to 32 bytes starting at byte 4
        assert!(call[4..16].iter().all(|&b| b == 0));
        assert!(call[16..24].iter().all(|&b| b == 0x11));
        // amount = 500 in the last 32 bytes
        assert_eq!(call[4 + 32 + 31], 244); // 500 = 0x01F4, last byte = 0xF4 = 244
    }

    // ── error cases ───────────────────────────────────────────────────────────

    #[test]
    fn fixed_bytes_empty_errors() {
        assert_eq!(
            AbiToken::fixed_bytes(b"").unwrap_err(),
            EthError::AbiInvalidBytesLength(0)
        );
    }

    #[test]
    fn fixed_bytes_too_long_errors() {
        let data = [0u8; 33];
        assert!(matches!(
            AbiToken::fixed_bytes(&data).unwrap_err(),
            EthError::AbiInvalidBytesLength(_)
        ));
    }
}
