# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

cowallet is an **AI-native MPC (Multi-Party Computation) crypto wallet** with three main components:

1. **Rust Backend** — MPC protocol implementation, API server, blockchain indexer, and worker services
2. **Flutter Mobile App** — iOS/Android client with Rust FFI bridge for cryptographic operations
3. **HTML Prototype** — Single-file static mockup (`prototype/index.html`) for UI/UX reference

**Core Technology**: DKLS23 threshold signature scheme (2-of-3 TSS) for ECDSA on secp256k1, targeting EVM chains (Ethereum, Base, Arbitrum, Optimism, BSC).

## Architecture

### 6-Layer Design

1. **L1 Client Layer** — Flutter mobile app with Rust FFI bridge
2. **L2 MPC Protocol** — DKLS23 (DKG, presign, sign, reshare) via `synedrion` crate
3. **L3 Key Shard Management** — 3 shards: device (Secure Enclave/Keystore), server (HSM), backup (offline)
4. **L4 Policy Engine** — Transaction limits, multi-approval, risk detection
5. **L5 Blockchain Layer** — EVM chains via `alloy`, ERC-4337 Account Abstraction
6. **L6 Backend Services** — Axum API, NATS messaging, PostgreSQL, Redis

### Rust Workspace Structure

```
crates/
├── mpc-core/           # L2+L3: DKLS23 protocol, shard management, transport (Noise_XX)
├── chain-evm/          # L5: EVM signer, transaction builder, ERC-20 tokens
├── policy-engine/      # L4: Rules engine, approval workflows, risk detection
├── ai-bridge/          # AI intent parsing, Claude API integration
├── storage-crypto/     # Encrypted storage, platform keychain access
└── ffi-mobile/         # Flutter Rust Bridge FFI exports

backend/
├── api-server/         # Axum HTTP API (port 3000)
├── mpc-relay/          # NATS-based MPC message relay
├── indexer/            # Blockchain event indexer
├── worker/             # Background job processor
└── migrations/         # SQL migrations (sqlx)

mobile/                 # Flutter app (Dart + Rust FFI)
prototype/              # Static HTML mockup (bilingual, phone-frame design)
```

## Development Commands

### Local Development (Native)

**Prerequisites**: PostgreSQL 16+, Redis 7+, NATS 2.x, Rust 1.75+, GCC 11+ (for aws-lc-sys)

```bash
# One-time initialization (starts PostgreSQL/Redis, creates DB, runs migrations)
make local-init

# Start services (requires NATS running separately)
# Terminal 1:
sudo systemctl start nats  # or: nats-server -js &

# Terminal 2:
make local-start            # Starts API server on localhost:3000

# Database migrations
make local-migrate          # Run pending migrations
sqlx migrate revert         # Revert last migration

# Build and test
make local-build            # cargo build --release
cargo test                  # Run all tests
cargo test -p mpc-core      # Test single crate

# Stop services
make local-stop             # Kill application processes

# Health check
curl http://localhost:3000/health
```

### Docker Development

```bash
make docker-up              # Start all services (API, PostgreSQL, Redis, NATS)
make docker-logs            # Follow logs
make docker-down            # Stop services
make docker-clean           # Stop and remove volumes
make docker-rebuild         # Full rebuild
```

### Flutter Mobile App

```bash
cd mobile

# Install dependencies
flutter pub get

# Run on device/emulator
flutter run

# Build
flutter build apk           # Android
flutter build ios           # iOS (requires macOS + Xcode)

# Run tests
flutter test

# Code generation (after modifying FFI)
flutter_rust_bridge_codegen generate
```

### Cargo Shortcuts

```bash
cargo fmt                   # Format code
cargo clippy -- -D warnings # Lint with warnings-as-errors
cargo run -p api-server     # Run specific binary
cargo check --workspace     # Fast type-check all crates
```

## Key Technical Details

### MPC Protocol Flow (DKLS23)

The `mpc-core` crate implements a 2-of-3 threshold signature scheme:

1. **DKG (Distributed Key Generation)**: 3 parties generate keypair shares without a trusted dealer
   - Round 1: Commitment phase
   - Round 2: Share distribution
   - Finalize: Each party gets a shard, public key derived
   
2. **Signing**: Any 2 of 3 parties can sign (sub-100ms online phase after offline presign)
   - Presign: Generate signing material (can be done in advance)
   - Sign: Combine presign data + message hash → ECDSA signature

3. **Reshare**: Refresh shards without changing the public key (rotation/recovery)

### FFI Bridge (Rust ↔ Dart)

Located in `crates/ffi-mobile/` and `mobile/lib/bridge/`:

- **Code generation**: `flutter_rust_bridge` v2 auto-generates Dart bindings from Rust functions
- **State management**: Global state in Rust (`LazyLock<Mutex<MpcState>>`), accessed via FFI
- **Error handling**: All FFI functions return `Result<T, String>` for graceful error propagation

Key FFI functions:
```rust
generate_wallet() -> Result<FfiWalletInfo, String>
dkg_session_new(party_index: u16) -> Result<FfiDkgSession, String>
dkg_generate_round1(session_id: String) -> Result<FfiRound1Result, String>
sign_hash(msg_hash: Vec<u8>) -> Result<Vec<u8>, String>
```

### EVM Integration

`chain-evm` crate uses `alloy` (ethers-rs successor):

- **MpcSigner**: Implements `alloy::network::Signer` trait, signs transactions via DKLS23
- **EIP-1559**: Type-2 transactions with dynamic fees
- **ERC-4337**: UserOperation support for account abstraction (gas sponsorship, batching)
- **Multi-chain**: Config for Ethereum, Base, Arbitrum, Optimism, BSC

### Policy Engine

`policy-engine` crate intercepts transactions before signing:

- **Rules**: Amount limits, daily spending caps, address whitelists/blacklists
- **Approval**: M-of-N multi-sig workflows (e.g., 2-of-3 for large transfers)
- **Risk**: Real-time anomaly detection (unusual destinations, phishing checks)

### Backend Services

- **api-server** (port 3000): Axum HTTP API with Tower middleware (CORS, tracing, rate limiting)
- **mpc-relay**: NATS-based pub-sub for MPC round messages between devices/server
- **indexer**: Tracks blockchain events (deposits, withdrawals) for balance updates
- **worker**: Background jobs (price feeds, pending tx monitoring, webhook notifications)

### Environment Variables

Critical vars (see `.env.example`):

```bash
DATABASE_URL=postgres://postgres@localhost:5432/cowallet
REDIS_URL=redis://localhost:6379
NATS_URL=nats://localhost:4222
RPC_URL=https://sepolia.base.org              # EVM RPC endpoint
CLAUDE_API_KEY=sk-ant-xxxxx                   # For AI features
```

## Testing Strategy

- **Unit tests**: Each crate has `#[cfg(test)]` modules testing core logic
- **Integration tests**: `backend/api-server/tests/` for HTTP endpoint testing
- **FFI tests**: `crates/ffi-mobile/src/lib.rs` tests Rust FFI functions directly
- **Flutter tests**: `mobile/test/` for widget and integration tests

Run all tests:
```bash
cargo test                  # Rust tests
cd mobile && flutter test   # Flutter tests
```

## Common Issues

### GCC Version Error (CentOS/Linux)

Error: "COMPILER BUG DETECTED" from aws-lc-sys

**Fix**: Upgrade to GCC 11+
```bash
# CentOS 7
sudo yum install centos-release-scl devtoolset-11-gcc
source scl_enable devtoolset-11

# CentOS 8/9
sudo yum install gcc-11
sudo update-alternatives --install /usr/bin/gcc gcc /usr/bin/gcc-11 100
```

### PostgreSQL Authentication Failed

Error: "password authentication failed for user postgres"

**Fix**: Already automated in `make local-init`, or manually:
```bash
sudo sed -i 's/^local.*all.*all.*peer/local   all             all                                     trust/' /var/lib/pgsql/16/data/pg_hba.conf
sudo systemctl restart postgresql-16
```

### FFI Code Generation

After modifying Rust FFI functions in `crates/ffi-mobile/src/api.rs`:

```bash
cd mobile
flutter_rust_bridge_codegen generate
flutter pub get
```

## Documentation

- **PLAN.md** — Full 6-layer architecture, technical stack, Gantt chart
- **IMPLEMENTATION.md** — Phase-by-phase implementation roadmap (currently Phase 2, 25% complete)
- **QUICK_START.md** — CentOS deployment quick reference (中文)
- **DEPLOY.md** — Detailed deployment guide for production
- **DOCKER.md** — Docker Compose setup and troubleshooting

## HTML Prototype (`prototype/index.html`)

A self-contained single-file (~3100 lines) mockup:

- **Structure**: CSS (lines ~10–1980), HTML (~1980–1982), JavaScript (~1983–3097)
- **Design**: Phone-frame mockup with paper/ink palette (Claude design language)
- **Bilingual**: CSS-driven `data-zh`/`data-en` attributes with `::before` pseudo-elements
- **Features**: Onboarding flow, wallet views, chat composer, intent detection (regex-based), automated demo (`demo.run()`)
- **Conventions**: `$` = `querySelector`, `data-nav="view"` triggers `setView()`, `data-onb="action"` for onboarding steps

Open directly in browser (no build step):
```bash
cd prototype
python3 -m http.server 8000  # or: npx serve .
```

## Project Status

- **Current Phase**: Phase 2 (25% complete)
- **Target**: W24 (6 months from start)
- **EVM Focus**: Ethereum, Base, Arbitrum, Optimism, BSC (Bitcoin, Solana, Cosmos deferred)
- **Completed**: Rust workspace, FFI bridge, DKG protocol, basic API server
- **In Progress**: Policy engine, ERC-4337 integration, mobile UI
