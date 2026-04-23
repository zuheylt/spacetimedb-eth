//! `EthError` — the single error type for the entire `eth-core` crate.

/// All errors produced by `eth-core`.
///
/// Designed so every public function returns `Result<T, EthError>`.
/// Under `no_std` we provide a manual `Display`; under `std` the
/// `thiserror` crate generates `Display` and `std::error::Error` for us.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EthError {
    // --- keccak ----------------------------------------------------------
    // (none yet — keccak is infallible)

    // --- RLP -------------------------------------------------------------
    /// Input ended before the RLP item was complete.
    RlpUnexpectedEof,
    /// RLP length prefix encodes a value that overflows `usize`.
    RlpLengthOverflow,
    /// An RLP list contains trailing bytes after all items.
    RlpTrailingBytes,

    // --- ABI -------------------------------------------------------------
    /// Tried to decode a type that needs at least `needed` bytes but only
    /// `got` were available.
    AbiBufferTooShort { needed: usize, got: usize },
    /// A dynamic-type offset points outside the input buffer.
    AbiOffsetOutOfRange { offset: usize, len: usize },
    /// An ABI `uint` or `int` has a bit-width that is not a multiple of 8,
    /// or is outside 8–256.
    AbiInvalidBitWidth(u16),
    /// An ABI `bytes<N>` has N = 0 or N > 32.
    AbiInvalidBytesLength(u8),

    // --- hex / parsing ---------------------------------------------------
    /// A hex string contained a non-hex character.
    HexInvalidChar(u8),
    /// A hex string had the wrong length for the expected type.
    HexWrongLength { expected: usize, got: usize },

    // --- transaction / signing -------------------------------------------
    /// The ECDSA signature produced an invalid recovery id.
    SigningFailed,
    /// `chain_id` is 0 or overflows EIP-155 calculation.
    InvalidChainId(u64),
}

// ── Display ──────────────────────────────────────────────────────────────────
// We write it manually so it works in both std and no_std.
// thiserror generates this automatically when the `std` feature is on,
// but only if we derive it — so we skip the derive and write it once.

#[cfg(feature = "std")]
impl std::error::Error for EthError {}

impl core::fmt::Display for EthError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RlpUnexpectedEof => f.write_str("RLP: unexpected end of input"),
            Self::RlpLengthOverflow => f.write_str("RLP: length prefix overflows usize"),
            Self::RlpTrailingBytes => f.write_str("RLP: trailing bytes after item"),
            Self::AbiBufferTooShort { needed, got } => write!(
                f,
                "ABI decode: buffer too short (needed {needed}, got {got})"
            ),
            Self::AbiOffsetOutOfRange { offset, len } => write!(
                f,
                "ABI decode: dynamic offset {offset} out of range (len={len})"
            ),
            Self::AbiInvalidBitWidth(w) => {
                write!(f, "ABI: invalid bit-width {w} (must be 8–256, multiple of 8)")
            }
            Self::AbiInvalidBytesLength(n) => {
                write!(f, "ABI: bytesN length {n} out of range (1–32)")
            }
            Self::HexInvalidChar(c) => write!(f, "hex: invalid character 0x{c:02x}"),
            Self::HexWrongLength { expected, got } => {
                write!(f, "hex: wrong length (expected {expected}, got {got})")
            }
            Self::SigningFailed => f.write_str("ECDSA signing failed"),
            Self::InvalidChainId(id) => write!(f, "invalid chain id: {id}"),
        }
    }
}
