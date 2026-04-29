use alloy_primitives::{B256, keccak256};

/// EIP-712 typed data signing for structured messages.
///
/// Used for: ERC-20 permit, ERC-2612, governance votes, off-chain orders.

/// Hash structured data according to EIP-712.
///
/// Computes: keccak256("\x19\x01" || domainSeparator || structHash)
pub fn hash_typed_data(domain_separator: &B256, struct_hash: &B256) -> B256 {
    let mut buf = Vec::with_capacity(2 + 32 + 32);
    buf.extend_from_slice(&[0x19, 0x01]);
    buf.extend_from_slice(domain_separator.as_ref());
    buf.extend_from_slice(struct_hash.as_ref());
    keccak256(&buf)
}
