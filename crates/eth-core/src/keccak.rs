//! Keccak-256 — the hash function underlying all of Ethereum.
//!
//! This is the original Keccak submission (2012), **not** NIST SHA3-256.
//! They share the same algorithm but differ in one padding byte (0x01 here,
//! 0x06 in SHA3). Ethereum was deployed before NIST finalised SHA3, so it
//! is permanently on the original variant.
//!
//! # How the sponge works (plain English)
//!
//! 1. **State** — a 5×5 grid of 64-bit numbers (1600 bits total), starts at zero.
//! 2. **Absorb** — XOR each 136-byte chunk of input into the first 136 bytes of
//!    the state, then scramble the whole state with the Keccak-f permutation.
//! 3. **Pad** — the last (possibly short) chunk is padded to exactly 136 bytes
//!    before absorbing.
//! 4. **Squeeze** — read the first 32 bytes of the final state as the digest.

// ── Constants ─────────────────────────────────────────────────────────────────

/// Bytes absorbed per permutation call (rate = 1600 − 2×256 bits).
const RATE: usize = 136;

/// Round constants for the ι (iota) step — one per round, 24 rounds total.
/// Derived from a LFSR defined in the Keccak spec; not arbitrary.
const RC: [u64; 24] = [
    0x0000000000000001, 0x0000000000008082, 0x800000000000808a, 0x8000000080008000,
    0x000000000000808b, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008a, 0x0000000000000088, 0x0000000080008009, 0x000000008000000a,
    0x000000008000808b, 0x800000000000008b, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800a, 0x800000008000000a,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
];

/// Bit-rotation amounts for the ρ (rho) step, indexed by `x + 5*y`.
/// Each lane gets rotated by a different amount so bits fan out across the state.
const RHO: [u32; 25] = [
     0,  1, 62, 28, 27,
    36, 44,  6, 55, 20,
     3, 10, 43, 25, 39,
    41, 45, 15, 21,  8,
    18,  2, 61, 56, 14,
];

// ── Keccak-f[1600] permutation ────────────────────────────────────────────────

/// The Keccak-f[1600] permutation — 24 rounds of five steps each.
///
/// `state` is the 5×5 grid of 64-bit lanes, stored flat as `state[x + 5*y]`.
fn keccak_f(state: &mut [u64; 25]) {
    for &rc in &RC {
        // ── θ (theta) — spread each column's parity to every neighbouring column
        //
        // Picture the 5 columns as 5 vertical stacks of 5 lanes.
        // C[x] = XOR of all lanes in column x.
        // D[x] = parity of column to the left XOR rotation of column to the right.
        // Every lane gets XOR'd with D[its column].
        //
        // Effect: every bit now depends on two full columns → avalanche starts.
        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        let mut d = [0u64; 5];
        for x in 0..5 {
            d[x] = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
        }
        for i in 0..25 {
            state[i] ^= d[i % 5];
        }

        // ── ρ (rho) + π (pi) — rotate lanes, then permute their positions
        //
        // ρ rotates each lane by a fixed amount (from RHO table) so bits
        // within a lane spread to different bit positions.
        //
        // π moves each lane to a new (x,y) position: A'[y, (2x+3y)%5] = A[x,y]
        //
        // We apply both in a single pass to avoid two temporary copies.
        let mut temp = [0u64; 25];
        for x in 0..5_usize {
            for y in 0..5_usize {
                let new_x = y;
                let new_y = (2 * x + 3 * y) % 5;
                temp[new_x + 5 * new_y] = state[x + 5 * y].rotate_left(RHO[x + 5 * y]);
            }
        }
        *state = temp;

        // ── χ (chi) — the only non-linear step; mixes bits within each row
        //
        // A'[x,y] = A[x,y] XOR ((NOT A[x+1,y]) AND A[x+2,y])
        //
        // This is the S-box that makes the function one-way: without χ,
        // Keccak would be a linear function and trivially invertible.
        for y in 0..5_usize {
            let b = y * 5;
            let row = [state[b], state[b+1], state[b+2], state[b+3], state[b+4]];
            for x in 0..5 {
                state[b + x] = row[x] ^ ((!row[(x + 1) % 5]) & row[(x + 2) % 5]);
            }
        }

        // ── ι (iota) — break symmetry between rounds with a round constant
        //
        // Without this, every round would do the same thing and 24 rounds
        // would be equivalent to 1.  XOR'ing a unique constant into state[0,0]
        // makes each round distinct.
        state[0] ^= rc;
    }
}

// ── Sponge helpers ────────────────────────────────────────────────────────────

/// XOR exactly `RATE` bytes from `block` into the state (the absorb step).
fn absorb(state: &mut [u64; 25], block: &[u8; RATE]) {
    // The state lanes are little-endian 64-bit integers.
    // zip pairs each state lane with its 8-byte block chunk — no index needed.
    for (lane, chunk) in state[..RATE / 8].iter_mut().zip(block.chunks_exact(8)) {
        *lane ^= u64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3],
            chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Hash any number of bytes with Keccak-256 and return a 32-byte digest.
///
/// This is the hash function used everywhere in Ethereum: address derivation,
/// transaction IDs, ABI function selectors, and event topic hashes.
///
/// # Example
///
/// ```
/// use eth_core::keccak::keccak256;
///
/// // The Keccak-256 of the empty string — a well-known constant in Ethereum.
/// let hash = keccak256(b"");
/// assert_eq!(hash[0], 0xc5);
/// assert_eq!(hash[31], 0x70);
/// assert_eq!(hash.len(), 32);
/// ```
pub fn keccak256(input: &[u8]) -> [u8; 32] {
    let mut state = [0u64; 25];
    let mut offset = 0;

    // Absorb all complete 136-byte blocks.
    while offset + RATE <= input.len() {
        let mut block = [0u8; RATE];
        block.copy_from_slice(&input[offset..offset + RATE]);
        absorb(&mut state, &block);
        keccak_f(&mut state);
        offset += RATE;
    }

    // Build the final (padded) block.
    //
    // Keccak padding rule: append byte 0x01 immediately after the data,
    // fill the rest with 0x00, then set the highest bit of the last byte.
    //
    // If the data ended exactly on a block boundary the padding is an entire
    // extra block — there is always at least one padding block.
    let mut last = [0u8; RATE];
    let tail = &input[offset..];
    last[..tail.len()].copy_from_slice(tail);
    last[tail.len()] ^= 0x01;
    last[RATE - 1] ^= 0x80;
    absorb(&mut state, &last);
    keccak_f(&mut state);

    // Squeeze: the first 32 bytes of the state are the digest.
    let mut out = [0u8; 32];
    for i in 0..4 {
        let bytes = state[i].to_le_bytes();
        out[i * 8..(i + 1) * 8].copy_from_slice(&bytes);
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    // Cross-check our implementation against the `sha3` crate's Keccak256.
    fn reference(input: &[u8]) -> [u8; 32] {
        use sha3::Digest;
        sha3::Keccak256::digest(input).into()
    }

    // ── Known-vector tests (published Ethereum test vectors) ──────────────────

    #[test]
    fn empty_input() {
        // From the Ethereum Yellow Paper, Appendix F.
        let expected = hex!("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");
        assert_eq!(keccak256(b""), expected);
    }

    #[test]
    fn abc() {
        let expected = hex!("4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45");
        assert_eq!(keccak256(b"abc"), expected);
    }

    #[test]
    fn transfer_event_topic() {
        // The topic-0 of every ERC-20 Transfer event.
        let expected = hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");
        assert_eq!(keccak256(b"Transfer(address,address,uint256)"), expected);
    }

    #[test]
    fn approval_event_topic() {
        // The topic-0 of every ERC-20 Approval event.
        let expected = hex!("8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925");
        assert_eq!(keccak256(b"Approval(address,address,uint256)"), expected);
    }

    // ── Cross-checks against the `sha3` reference crate ──────────────────────

    #[test]
    fn cross_check_various_lengths() {
        // Covers: empty, sub-block, exact block boundary, multi-block, and
        // the tricky "last byte carries both 0x01 and 0x80" case (len = 135).
        for len in [0, 1, 55, 135, 136, 137, 271, 272, 273, 500] {
            let input: Vec<u8> = (0..len).map(|i| i as u8).collect();
            assert_eq!(
                keccak256(&input),
                reference(&input),
                "mismatch at input length {len}"
            );
        }
    }
}
