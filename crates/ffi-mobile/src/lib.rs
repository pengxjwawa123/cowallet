mod frb_generated; /* AUTO INJECTED BY flutter_rust_bridge. This line may not be accurate, and you can change it according to your needs. */
// FFI bridge for Flutter ↔ Rust via flutter_rust_bridge.
//
// This crate re-exports FFI-safe types and functions from the workspace crates.
// The Dart side communicates through these entry points only.
// Secret material (private keys, key shares) NEVER crosses the FFI boundary.
// Dart receives opaque handles (UUIDs) and public data (addresses, balances, tx hashes).

pub mod api;
mod state;

#[cfg(test)]
mod tests;
