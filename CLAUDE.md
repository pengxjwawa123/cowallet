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

### API Route Structure

```
/health, /ready, /live, /metrics   — Public probes
/api/v1/auth/*                     — Public (auth rate limit: 5 req/min)
/api/v1/price/*                    — Public
/api/v1/mpc/*                      — Protected + strict rate limit (10 req/min)
/api/v1/tx/*                       — Protected
/api/v1/policy/*                   — Protected
/api/v1/ai/*                       — Protected
/api/v1/yield/*                    — Protected
/api/v1/shards/*                   — Protected
```

Protected routes require JWT via `Authorization: Bearer <token>`. Auth middleware in `backend/api-server/src/middleware/auth.rs`.

### Mobile App Architecture

```
mobile/lib/
├── api/          # HTTP API clients (auth, mpc, ai, shards, wallet, policy)
├── bridge/       # Rust FFI bridge (flutter_rust_bridge v2 generated)
├── config/       # API base URL, timeouts
├── network/      # Dio HTTP client with auto-token injection
├── platform/     # Native platform channels (Secure Enclave, StrongBox, biometrics)
├── state/        # App state management
├── views/        # UI screens (home, wallet, chat, send, receive, keys, settings)
└── widgets/      # Reusable UI components
```

The mobile app uses Dio for HTTP. Token is auto-injected from secure storage via interceptor (`mobile/lib/network/dio_client.dart`).

## Development Commands

### Quick Start (macOS with Docker for infra)

Uses `Makefile.local` — starts Postgres/Redis/NATS in Docker, runs API natively:

```bash
make -f Makefile.local up       # Start infra (Postgres:5433, Redis:6380, NATS:4223)
make -f Makefile.local migrate  # Run migrations
make -f Makefile.local dev      # up + migrate + cargo run api-server
make -f Makefile.local down     # Stop infra
```

### Local Development (CentOS/Linux with native services)

**Prerequisites**: PostgreSQL 16+, Redis 7+, NATS 2.x, Rust stable, GCC 11+ (for aws-lc-sys)

```bash
make local-init                 # One-time: start PG/Redis, create DB, run migrations
make local-start                # cargo run --release --bin api-server
make local-migrate              # sqlx migrate run
make local-stop                 # Kill app processes
```

### Docker (full stack)

```bash
make docker-up                  # Start all services (API + infra)
make docker-logs                # Follow logs
make docker-down                # Stop
make docker-clean               # Stop + remove volumes
make docker-rebuild             # Rebuild from scratch
```

### Build & Test

```bash
cargo check --workspace         # Fast type-check
cargo build --release           # Full build
cargo test                      # All tests
cargo test -p mpc-core          # Single crate
cargo fmt                       # Format
cargo clippy -- -D warnings     # Lint
cargo run -p api-server         # Run API server (dev mode)
```

### Flutter Mobile

```bash
cd mobile
flutter pub get                 # Install deps
flutter run                     # Run on device/emulator
flutter test                    # Run tests
flutter_rust_bridge_codegen generate  # Regenerate FFI after Rust changes
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

- **api-server** (port 3000): Axum HTTP API with Tower middleware stack (CORS, tracing, rate limiting, security headers, request body limit 10MB, 30s timeout). Can start in degraded mode without DB.
- **mpc-relay**: NATS-based pub-sub for MPC round messages between devices/server
- **indexer**: Tracks blockchain events (deposits, withdrawals) for balance updates
- **worker**: Background jobs (price feeds, pending tx monitoring, webhook notifications)

AppState (`backend/api-server/src/state.rs`) holds the DB pool, HTTP client, Claude client, rate limiter, circuit breakers, and caches. Migrations auto-run on startup via `sqlx::migrate!`.

### Environment Variables

Critical vars (see `.env.example`):

```bash
DATABASE_URL=postgres://postgres@localhost:5432/cowallet
REDIS_URL=redis://localhost:6379
NATS_URL=nats://localhost:4222
RPC_URL=https://sepolia.base.org              # EVM RPC endpoint
RPC_WS_URL=wss://sepolia.base.org             # WebSocket RPC
ANTHROPIC_API_KEY=sk-ant-xxxxx                # For AI features (Claude client)
JWT_SECRET=...                                # Min 32 chars for token signing
ENCRYPTION_KEY=...                            # 32-byte hex for shard encryption
CORS_ALLOWED_ORIGINS=http://localhost:3000    # Comma-separated origins
```

Note: Docker Compose maps non-standard host ports (Postgres:5433, Redis:6380, NATS:4223) to avoid conflicts with local services. The `Makefile.local` `migrate` target uses `postgres://postgres:postgres@localhost:5433/cowallet`.

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

## Database

Migrations live in `backend/migrations/` (sequential numbered SQL files). The `mpc_sessions` and `mpc_messages` tables are the core MPC state. Key tables: `users`, `mpc_sessions`, `mpc_messages`, `shard_metadata`, `transactions`, `policies`.

Migrations auto-run on api-server startup. To run manually:
```bash
sqlx migrate run --source backend/migrations
```

## Deployment

Production deploys to ECS via GitHub Actions (`.github/workflows/deploy-ecs.yml`). The Dockerfile builds all workspace binaries; the specific binary is selected via `command:` in docker-compose.

## HTML Prototype (`prototype/index.html`)

A self-contained single-file (~3100 lines) mockup. Open directly in browser (no build step):
```bash
cd prototype && python3 -m http.server 8000
```
