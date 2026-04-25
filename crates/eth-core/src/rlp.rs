//! RLP (Recursive Length Prefix) encoding and decoding.
//!
//! RLP is Ethereum's serialization format. Every transaction, block header,
//! and receipt is RLP-encoded before being hashed or transmitted.
//!
//! The format has exactly two types:
//! - **Byte string** — any sequence of bytes (empty, one byte, or many).
//! - **List** — an ordered sequence of items, each of which is itself either
//!   a byte string or a list.
//!
//! # Quick example
//!
//! ```
//! use eth_core::rlp::{RlpItem, decode_exact};
//!
//! // Build and encode a simple list
//! let item = RlpItem::List(vec![
//!     RlpItem::from_uint(1),
//!     RlpItem::Bytes(b"dog".to_vec()),
//! ]);
//! let encoded = item.encode();
//!
//! // Decode it back and check round-trip
//! let decoded = decode_exact(&encoded).unwrap();
//! assert_eq!(item, decoded);
//! ```

#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec};

use crate::error::EthError;

// ── Data model ────────────────────────────────────────────────────────────────

/// An RLP value: either a byte string or a list of values.
///
/// This enum directly mirrors the RLP data model. Build one up,
/// then call [`encode`](RlpItem::encode) to serialise it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RlpItem {
    /// A raw byte string (may be empty).
    Bytes(Vec<u8>),
    /// An ordered list of RLP items.
    List(Vec<RlpItem>),
}

impl RlpItem {
    // ── Constructors ─────────────────────────────────────────────────────────

    /// Create a `Bytes` item from a `u64`.
    ///
    /// Ethereum encodes integers as **big-endian, no leading zeros**.
    /// `0` becomes the empty byte string `[]`; `256` becomes `[0x01, 0x00]`.
    ///
    /// Use this for: nonce, gas price, gas limit, chain id.
    ///
    /// # Example
    ///
    /// ```
    /// use eth_core::rlp::RlpItem;
    ///
    /// assert_eq!(RlpItem::from_uint(0),    RlpItem::Bytes(vec![]));
    /// assert_eq!(RlpItem::from_uint(1),    RlpItem::Bytes(vec![0x01]));
    /// assert_eq!(RlpItem::from_uint(1024), RlpItem::Bytes(vec![0x04, 0x00]));
    /// ```
    pub fn from_uint(n: u64) -> Self {
        RlpItem::Bytes(trim_be_zeros(&n.to_be_bytes()))
    }

    /// Create a `Bytes` item from a `u128`.
    ///
    /// Use this for ETH values (wei), which can exceed `u64::MAX`
    /// (though they fit comfortably in `u128`).
    ///
    /// # Example
    ///
    /// ```
    /// use eth_core::rlp::RlpItem;
    ///
    /// // 1 ETH in wei = 10^18
    /// let one_eth: u128 = 1_000_000_000_000_000_000;
    /// let item = RlpItem::from_uint128(one_eth);
    /// assert!(matches!(item, RlpItem::Bytes(_)));
    /// ```
    pub fn from_uint128(n: u128) -> Self {
        RlpItem::Bytes(trim_be_zeros(&n.to_be_bytes()))
    }

    // ── Encoding ─────────────────────────────────────────────────────────────

    /// Encode this item into bytes following the RLP specification.
    ///
    /// # Example
    ///
    /// ```
    /// use eth_core::rlp::RlpItem;
    ///
    /// // The string "dog" encodes as [0x83, b'd', b'o', b'g']
    /// let encoded = RlpItem::Bytes(b"dog".to_vec()).encode();
    /// assert_eq!(encoded, vec![0x83, b'd', b'o', b'g']);
    ///
    /// // An empty list encodes as [0xc0]
    /// let empty_list = RlpItem::List(vec![]).encode();
    /// assert_eq!(empty_list, vec![0xc0]);
    /// ```
    pub fn encode(&self) -> Vec<u8> {
        match self {
            RlpItem::Bytes(bytes) => encode_bytes(bytes),
            RlpItem::List(items) => {
                let payload: Vec<u8> = items.iter().flat_map(|i| i.encode()).collect();
                let mut out = length_prefix(payload.len(), 0xc0);
                out.extend_from_slice(&payload);
                out
            }
        }
    }
}

// ── Encoding helpers ──────────────────────────────────────────────────────────

/// Strip leading zero bytes from a big-endian byte slice.
/// Produces an empty `Vec` for an all-zero input, matching Ethereum's
/// "no leading zeros" integer encoding rule.
fn trim_be_zeros(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().copied().skip_while(|&b| b == 0).collect()
}

fn encode_bytes(bytes: &[u8]) -> Vec<u8> {
    // RLP Rule 1: a single byte in [0x00, 0x7f] is its own encoding.
    if bytes.len() == 1 && bytes[0] < 0x80 {
        return bytes.to_vec();
    }
    // RLP Rules 2 & 3: length-prefixed string (offset 0x80 for strings).
    let mut out = length_prefix(bytes.len(), 0x80);
    out.extend_from_slice(bytes);
    out
}

/// Build the length prefix for a payload of `len` bytes.
///
/// - `len` ≤ 55: one byte = `offset + len`
/// - `len` > 55: `offset + 55 + byte_width(len)`, then `len` in big-endian
///
/// `offset` is 0x80 for byte strings and 0xc0 for lists.
fn length_prefix(len: usize, offset: u8) -> Vec<u8> {
    if len <= 55 {
        vec![offset + len as u8]
    } else {
        let len_bytes = usize_to_be_bytes(len);
        let mut out = vec![offset + 55 + len_bytes.len() as u8];
        out.extend_from_slice(&len_bytes);
        out
    }
}

/// Encode a `usize` as big-endian bytes with no leading zeros.
fn usize_to_be_bytes(n: usize) -> Vec<u8> {
    // Cast to u64 so the output width is platform-independent.
    trim_be_zeros(&(n as u64).to_be_bytes())
}

// ── Decoding ──────────────────────────────────────────────────────────────────

/// Decode one RLP item from the start of `input`.
///
/// Returns `(item, remaining)` — the decoded item and any bytes that
/// follow it in `input`.  Call this in a loop to decode sequential items:
///
/// ```
/// use eth_core::rlp::decode;
///
/// let input = [0x01u8, 0x02, 0x03]; // three single-byte RLP items
/// let (a, rest) = decode(&input).unwrap();
/// let (b, rest) = decode(rest).unwrap();
/// let (c, rest) = decode(rest).unwrap();
/// assert!(rest.is_empty());
/// ```
pub fn decode(input: &[u8]) -> Result<(RlpItem, &[u8]), EthError> {
    let &first = input.first().ok_or(EthError::RlpUnexpectedEof)?;

    match first {
        // ── Single byte [0x00, 0x7f] — the byte itself ────────────────────
        0x00..=0x7f => Ok((RlpItem::Bytes(vec![first]), &input[1..])),

        // ── Short string [0x80, 0xb7] — length = first - 0x80 ────────────
        0x80..=0xb7 => {
            let len = (first - 0x80) as usize;
            let end = 1 + len;
            if input.len() < end {
                return Err(EthError::RlpUnexpectedEof);
            }
            Ok((RlpItem::Bytes(input[1..end].to_vec()), &input[end..]))
        }

        // ── Long string [0xb8, 0xbf] — next (first-0xb7) bytes are length
        0xb8..=0xbf => {
            let (payload, rest) = decode_long_payload(input, 0xb7)?;
            Ok((RlpItem::Bytes(payload.to_vec()), rest))
        }

        // ── Short list [0xc0, 0xf7] — payload length = first - 0xc0 ──────
        0xc0..=0xf7 => {
            let len = (first - 0xc0) as usize;
            let end = 1 + len;
            if input.len() < end {
                return Err(EthError::RlpUnexpectedEof);
            }
            let items = decode_list_payload(&input[1..end])?;
            Ok((RlpItem::List(items), &input[end..]))
        }

        // ── Long list [0xf8, 0xff] — next (first-0xf7) bytes are length ──
        0xf8..=0xff => {
            let (payload, rest) = decode_long_payload(input, 0xf7)?;
            Ok((RlpItem::List(decode_list_payload(payload)?), rest))
        }
    }
}

/// Decode exactly one RLP item from `input`, returning an error if bytes
/// remain after it.
///
/// # Example
///
/// ```
/// use eth_core::rlp::{RlpItem, decode_exact};
///
/// let bytes = RlpItem::Bytes(b"dog".to_vec()).encode();
/// let item = decode_exact(&bytes).unwrap();
/// assert_eq!(item, RlpItem::Bytes(b"dog".to_vec()));
/// ```
pub fn decode_exact(input: &[u8]) -> Result<RlpItem, EthError> {
    let (item, rest) = decode(input)?;
    if !rest.is_empty() {
        return Err(EthError::RlpTrailingBytes);
    }
    Ok(item)
}

// ── Decoding helpers ──────────────────────────────────────────────────────────

/// Extract the payload and remaining bytes for a "long" RLP item
/// (strings ≥ 56 bytes or lists with payload ≥ 56 bytes).
///
/// `base` is 0xb7 for strings and 0xf7 for lists; it is subtracted from
/// the first byte to get the number of length bytes that follow.
fn decode_long_payload(input: &[u8], base: u8) -> Result<(&[u8], &[u8]), EthError> {
    let len_of_len = (input[0] - base) as usize;
    if input.len() < 1 + len_of_len {
        return Err(EthError::RlpUnexpectedEof);
    }
    let len = be_bytes_to_usize(&input[1..1 + len_of_len])?;
    let start = 1 + len_of_len;
    let end = start + len;
    if input.len() < end {
        return Err(EthError::RlpUnexpectedEof);
    }
    Ok((&input[start..end], &input[end..]))
}

/// Decode all RLP items packed sequentially into `payload`.
fn decode_list_payload(mut payload: &[u8]) -> Result<Vec<RlpItem>, EthError> {
    let mut items = Vec::new();
    while !payload.is_empty() {
        let (item, rest) = decode(payload)?;
        items.push(item);
        payload = rest;
    }
    Ok(items)
}

/// Interpret `bytes` as an unsigned big-endian integer that fits in `usize`.
fn be_bytes_to_usize(bytes: &[u8]) -> Result<usize, EthError> {
    if bytes.is_empty() {
        return Err(EthError::RlpUnexpectedEof);
    }
    let mut n: usize = 0;
    for &b in bytes {
        n = n
            .checked_mul(256)
            .and_then(|v| v.checked_add(b as usize))
            .ok_or(EthError::RlpLengthOverflow)?;
    }
    Ok(n)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    // Helper: encode shorthand
    fn enc(item: RlpItem) -> Vec<u8> {
        item.encode()
    }

    // ── Known-vector tests (Ethereum wiki / ethereum/tests repository) ─────

    #[test]
    fn encode_empty_string() {
        assert_eq!(enc(RlpItem::Bytes(vec![])), hex!("80"));
    }

    #[test]
    fn encode_single_byte_below_0x80() {
        // Single bytes in [0x00, 0x7f] are their own encoding.
        assert_eq!(enc(RlpItem::Bytes(vec![0x00])), hex!("00"));
        assert_eq!(enc(RlpItem::Bytes(vec![0x01])), hex!("01"));
        assert_eq!(enc(RlpItem::Bytes(vec![0x7f])), hex!("7f"));
    }

    #[test]
    fn encode_single_byte_0x80_needs_prefix() {
        // 0x80 is ≥ 0x80 so it gets a length prefix.
        assert_eq!(enc(RlpItem::Bytes(vec![0x80])), hex!("8180"));
    }

    #[test]
    fn encode_dog() {
        assert_eq!(enc(RlpItem::Bytes(b"dog".to_vec())), hex!("83646f67"));
    }

    #[test]
    fn encode_empty_list() {
        assert_eq!(enc(RlpItem::List(vec![])), hex!("c0"));
    }

    #[test]
    fn encode_cat_dog_list() {
        let item = RlpItem::List(vec![
            RlpItem::Bytes(b"cat".to_vec()),
            RlpItem::Bytes(b"dog".to_vec()),
        ]);
        assert_eq!(item.encode(), hex!("c88363617483646f67"));
    }

    #[test]
    fn encode_nested_empty_lists() {
        // [[], [[]], [[], [[]]]]  from the Ethereum wiki
        let item = RlpItem::List(vec![
            RlpItem::List(vec![]),
            RlpItem::List(vec![RlpItem::List(vec![])]),
            RlpItem::List(vec![
                RlpItem::List(vec![]),
                RlpItem::List(vec![RlpItem::List(vec![])]),
            ]),
        ]);
        assert_eq!(item.encode(), hex!("c7c0c1c0c3c0c1c0"));
    }

    // ── Integer encoding ──────────────────────────────────────────────────

    #[test]
    fn from_uint_zero() {
        // 0 → empty byte string → 0x80
        assert_eq!(enc(RlpItem::from_uint(0)), hex!("80"));
    }

    #[test]
    fn from_uint_one() {
        assert_eq!(enc(RlpItem::from_uint(1)), hex!("01"));
    }

    #[test]
    fn from_uint_0x7f() {
        assert_eq!(enc(RlpItem::from_uint(0x7f)), hex!("7f"));
    }

    #[test]
    fn from_uint_0x80() {
        // 128 needs a length prefix since the byte value is ≥ 0x80.
        assert_eq!(enc(RlpItem::from_uint(0x80)), hex!("8180"));
    }

    #[test]
    fn from_uint_1024() {
        assert_eq!(enc(RlpItem::from_uint(1024)), hex!("820400"));
    }

    // ── Round-trip tests ──────────────────────────────────────────────────

    #[test]
    fn roundtrip_bytes() {
        for payload in [
            vec![],
            vec![0x00],
            vec![0x7f],
            vec![0x80],
            b"hello world".to_vec(),
            vec![0u8; 56],   // triggers long-string path
        ] {
            let item = RlpItem::Bytes(payload);
            assert_eq!(decode_exact(&item.encode()).unwrap(), item);
        }
    }

    #[test]
    fn roundtrip_list() {
        let item = RlpItem::List(vec![
            RlpItem::from_uint(0),
            RlpItem::from_uint(1_000_000),
            RlpItem::Bytes(b"data".to_vec()),
            RlpItem::List(vec![
                RlpItem::Bytes(vec![0xff]),
            ]),
        ]);
        assert_eq!(decode_exact(&item.encode()).unwrap(), item);
    }

    #[test]
    fn roundtrip_all_uint_values() {
        for n in [0u64, 1, 127, 128, 255, 256, 65535, u64::MAX] {
            let item = RlpItem::from_uint(n);
            assert_eq!(decode_exact(&item.encode()).unwrap(), item);
        }
    }

    // ── Error cases ───────────────────────────────────────────────────────

    #[test]
    fn decode_empty_input_errors() {
        assert_eq!(decode(b""), Err(EthError::RlpUnexpectedEof));
    }

    #[test]
    fn decode_truncated_string_errors() {
        // 0x83 says "3-byte string follows" but there are no bytes after it.
        assert_eq!(decode(&[0x83]), Err(EthError::RlpUnexpectedEof));
    }

    #[test]
    fn decode_trailing_bytes_errors() {
        // 0x01 is a complete single-byte item; 0x02 is an unexpected extra byte.
        assert_eq!(decode_exact(&[0x01, 0x02]), Err(EthError::RlpTrailingBytes));
    }

    #[test]
    fn sequential_decode() {
        // Decode two items packed back-to-back.
        let mut buf = RlpItem::from_uint(42).encode();
        buf.extend(RlpItem::Bytes(b"hi".to_vec()).encode());

        let (first, rest) = decode(&buf).unwrap();
        let (second, rest) = decode(rest).unwrap();
        assert!(rest.is_empty());
        assert_eq!(first,  RlpItem::from_uint(42));
        assert_eq!(second, RlpItem::Bytes(b"hi".to_vec()));
    }
}
