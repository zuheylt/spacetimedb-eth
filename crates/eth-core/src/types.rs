//! Fixed-size byte-array newtypes: `Address`, `TxHash`, `Bytes32`.
//!
//! All three types share the same interface:
//! - `from_bytes` / `as_bytes` — construct from or read as a raw array
//! - `Display` — prints lowercase `0x`-prefixed hex
//! - `FromStr` — parses `0x…` or bare hex; returns `EthError` on bad input
//!
//! # Example
//!
//! ```
//! use eth_core::types::TxHash;
//!
//! let hash = TxHash::from_bytes([0xdd; 32]);
//! let s = hash.to_string();           // "0xdddd...dddd"
//! let back: TxHash = s.parse().unwrap();
//! assert_eq!(hash, back);
//! ```

use core::fmt;
use core::str::FromStr;

use crate::error::EthError;

// ── Hex helper ───────────────────────────────────────────────────────────────

/// Converts one ASCII hex character to its 0–15 nibble value.
fn hex_nibble(b: u8) -> Result<u8, EthError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(EthError::HexInvalidChar(b)),
    }
}

// ── Macro: generates Address, TxHash, Bytes32 ────────────────────────────────
//
// All three types are structurally identical (newtype over [u8; N]).
// The macro stamps out the same impl for each size so there is no repetition.

macro_rules! fixed_bytes {
    ($name:ident, $n:expr, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name([u8; $n]);

        impl $name {
            /// Constructs from a raw byte array.
            pub fn from_bytes(bytes: [u8; $n]) -> Self {
                Self(bytes)
            }

            /// Returns a reference to the underlying byte array.
            pub fn as_bytes(&self) -> &[u8; $n] {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("0x")?;
                for byte in &self.0 {
                    write!(f, "{byte:02x}")?;
                }
                Ok(())
            }
        }

        impl FromStr for $name {
            type Err = EthError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let hex = s.strip_prefix("0x").unwrap_or(s);
                let expected = $n * 2;
                if hex.len() != expected {
                    return Err(EthError::HexWrongLength { expected, got: hex.len() });
                }
                let mut bytes = [0u8; $n];
                for (i, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
                    bytes[i] = hex_nibble(pair[0])? << 4 | hex_nibble(pair[1])?;
                }
                Ok(Self(bytes))
            }
        }
    };
}

fixed_bytes!(
    Address,
    20,
    "20-byte Ethereum address — the last 20 bytes of a Keccak-256 public-key hash."
);
fixed_bytes!(
    TxHash,
    32,
    "32-byte transaction or block hash (Keccak-256 output)."
);
fixed_bytes!(
    Bytes32,
    32,
    "Generic 32-byte value — used for storage slots, event topics, and other 256-bit quantities."
);

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(feature = "std"))]
    use alloc::string::ToString;

    // ── Address ───────────────────────────────────────────────────────────────

    #[test]
    fn address_from_and_as_bytes() {
        let raw = [1u8; 20];
        assert_eq!(Address::from_bytes(raw).as_bytes(), &raw);
    }

    #[test]
    fn address_display_zero() {
        let addr = Address::from_bytes([0u8; 20]);
        assert_eq!(addr.to_string(), "0x0000000000000000000000000000000000000000");
    }

    #[test]
    fn address_from_str_with_prefix() {
        let addr: Address = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".parse().unwrap();
        assert_eq!(addr.as_bytes()[0], 0xd8);
        assert_eq!(addr.as_bytes()[1], 0xda);
    }

    #[test]
    fn address_from_str_without_prefix() {
        let addr: Address = "d8dA6BF26964aF9D7eEd9e03E53415D37aA96045".parse().unwrap();
        assert_eq!(addr.as_bytes()[0], 0xd8);
    }

    #[test]
    fn address_wrong_length() {
        let err = "0x1234".parse::<Address>().unwrap_err();
        assert_eq!(err, EthError::HexWrongLength { expected: 40, got: 4 });
    }

    #[test]
    fn address_invalid_char() {
        let err = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA9604Z".parse::<Address>().unwrap_err();
        assert!(matches!(err, EthError::HexInvalidChar(_)));
    }

    #[test]
    fn address_roundtrip() {
        let raw = [0xab; 20];
        let addr = Address::from_bytes(raw);
        let back: Address = addr.to_string().parse().unwrap();
        assert_eq!(addr, back);
    }

    // ── TxHash ────────────────────────────────────────────────────────────────

    #[test]
    fn txhash_display_zero() {
        let hash = TxHash::from_bytes([0u8; 32]);
        assert_eq!(
            hash.to_string(),
            "0x0000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn txhash_from_str_known_value() {
        // ERC-20 Transfer event topic — a well-known Keccak-256 output.
        let hash: TxHash = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
            .parse()
            .unwrap();
        assert_eq!(hash.as_bytes()[0], 0xdd);
        assert_eq!(hash.as_bytes()[31], 0xef);
    }

    #[test]
    fn txhash_wrong_length() {
        let err = "0x1234".parse::<TxHash>().unwrap_err();
        assert_eq!(err, EthError::HexWrongLength { expected: 64, got: 4 });
    }

    #[test]
    fn txhash_roundtrip() {
        let raw = [0xcd; 32];
        let hash = TxHash::from_bytes(raw);
        let back: TxHash = hash.to_string().parse().unwrap();
        assert_eq!(hash, back);
    }

    // ── Bytes32 ───────────────────────────────────────────────────────────────

    #[test]
    fn bytes32_roundtrip() {
        let raw = [0x42; 32];
        let b = Bytes32::from_bytes(raw);
        let back: Bytes32 = b.to_string().parse().unwrap();
        assert_eq!(b, back);
    }

    #[test]
    fn bytes32_invalid_char() {
        let err = "0xGG42424242424242424242424242424242424242424242424242424242424242"
            .parse::<Bytes32>()
            .unwrap_err();
        assert!(matches!(err, EthError::HexInvalidChar(_)));
    }
}
