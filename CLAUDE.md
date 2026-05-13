# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

cowallet is an **AI-native MPC (Multi-Party Computation) crypto wallet** with three main components:

1. **Rust Backend** — MPC protocol implementation, API server, blockchain indexer, and worker services
2. **Flutter Mobile App** — iOS/Android client with Rust FFI bridge for cryptographic operations
3. **HTML Prototype** — Single-file static mockup (`prototype/index.html`) for UI/UX reference

**Core Technology**: DKLS23 threshold signature scheme (2-of-3 TSS) for ECDSA on secp256k1, targeting EVM chains (Ethereum, Base, Arbitrum, Optimism, BSC, Polygon).

## Architecture

### 6-Layer Design

1. **L1 Client Layer** — Flutter mobile app with Rust FFI bridge
2. **L2 MPC Protocol** — DKLS23 (DKG, presign, sign, reshare) via custom implementation in `mpc-core`
3. **L3 Key Shard Management** — 3 shards: device (Secure Enclave/Keystore), server (HSM), backup (offline)
4. **L4 Policy Engine** — Transaction limits, multi-approval, risk detection
5. **L5 Blockchain Layer** — EVM chains via `alloy`, ERC-4337 Account Abstraction
6. **L6 Backend Services** — Axum API, NATS messaging, PostgreSQL, Redis

### Rust Workspace Structure

```
crates/
├── mpc-core/           # L2+L3: DKLS23 protocol (dkg, presign, sign, reshare), shard mgmt, Noise_XX transport
├── chain-evm/          # L5: EVM signer (alloy), transaction builder, ERC-20 tokens
├── policy-engine/      # L4: Rules engine, approval workflows, risk detection
├── ai-bridge/          # AI intent parsing integration
├── storage-crypto/     # Encrypted storage, platform keychain access
└── ffi-mobile/         # Flutter Rust Bridge FFI exports

backend/
├── api-server/         # Axum HTTP API (port 3000)
├── mpc-relay/          # NATS-based MPC message relay
├── indexer/            # Blockchain event indexer
├── worker/             # Background job processor
└── migrations/         # SQL migrations (sqlx, sequential numbered .sql files)

mobile/                 # Flutter app (Dart + Rust FFI)
prototype/              # Static HTML mockup (bilingual, phone-frame design)
```

### API Route Structure

```
/health, /ready, /live, /metrics   — Public probes
/ws/mpc/:session_id                — WebSocket (MPC rounds, merged at root)
/api/v1/auth/*                     — Public (auth rate limit: 5 req/min)
/api/v1/price/*                    — Public
/api/v1/chains/*                   — Public
/api/v1/mpc/*                      — Protected + strict rate limit (10 req/min)
/api/v1/tx/*                       — Protected
/api/v1/balance/*                  — Protected (Covalent API)
/api/v1/wallets/*                  — Protected
/api/v1/policy/*                   — Protected
/api/v1/ai/*                       — Protected
/api/v1/yield/*                    — Protected
/api/v1/shards/*                   — Protected
```

Protected routes require JWT via `Authorization: Bearer <token>`. Auth middleware in `backend/api-server/src/middleware/auth.rs`.

### Backend Services

- **api-server** (port 3000): Axum HTTP with Tower middleware (CORS, tracing, rate limiting, security headers, 10MB body limit, 30s timeout). Requires DB to start. Graceful shutdown on SIGINT/SIGTERM.
- **mpc-relay**: NATS pub-sub for MPC round messages. Falls back to DB polling if NATS unavailable.
- **indexer**: Tracks blockchain events (deposits, withdrawals) for balance updates.
- **worker**: Background jobs (price feeds, pending tx monitoring).

AppState (`backend/api-server/src/state.rs`) holds: DB pool, per-chain RPC URLs, HTTP client, AI client (DeepSeek), NATS, rate limiter, circuit breakers, metrics, MPC participant, presign manager, Covalent API key.

### AI Integration

The AI client (`backend/api-server/src/services/claude.rs`) uses **DeepSeek** via OpenAI-compatible API — not Anthropic despite the filename. Configured via `DEEPSEEK_API_KEY`, `DEEPSEEK_BASE_URL`, `DEEPSEEK_MODEL`.

### MPC Protocol Flow (DKLS23)

The `mpc-core` crate implements a 2-of-3 threshold signature scheme:

1. **DKG**: 3 parties generate keypair shares (Round 1: commitment, Round 2: share distribution, Finalize: each party gets a shard)
2. **Presign**: Generate signing material offline (can be done in advance, managed by `PresignManager`)
3. **Sign**: Combine presign data + message hash -> ECDSA signature (sub-100ms online phase)
4. **Reshare**: Refresh shards without changing the public key

Server-side MPC participant (`services/mpc_participant/`) manages shard storage with AES-GCM encryption and background presignature generation.

### FFI Bridge (Rust <-> Dart)

Located in `crates/ffi-mobile/` and `mobile/lib/bridge/`:
- `flutter_rust_bridge` v2 generates Dart bindings from Rust functions
- Global state in Rust (`LazyLock<Mutex<MpcState>>`), accessed via FFI
- All FFI functions return `Result<T, String>`
- After modifying `crates/ffi-mobile/src/api.rs`: run `flutter_rust_bridge_codegen generate` then `flutter pub get`

## Development Commands

### Quick Start (macOS with Docker for infra)

```bash
make -f Makefile.local up       # Start infra via docker-compose (Postgres:5433, Redis:6380, NATS:4223)
make -f Makefile.local migrate  # Run migrations
make -f Makefile.local dev      # up + migrate + cargo run api-server
make -f Makefile.local down     # Stop infra
```

### Local Development (CentOS/Linux with native services)

Prerequisites: PostgreSQL 16+, Redis 7+, NATS 2.x, Rust stable, GCC 11+ (for aws-lc-sys)

```bash
make local-init                 # One-time: start PG/Redis, configure auth, create DB, run migrations
make local-start                # cargo run --release --bin api-server
make local-migrate              # sqlx migrate run --source backend/migrations
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
flutter pub get
flutter run
flutter test
flutter_rust_bridge_codegen generate  # Regenerate FFI after Rust changes
```

## Environment Variables

Critical vars (see `.env.example`):

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/cowallet
REDIS_URL=redis://localhost:6379
NATS_URL=nats://localhost:4222
RPC_URL=https://sepolia.base.org
ENCRYPTION_KEY=<64-hex-chars>           # 32-byte AES key for shard encryption (required)
JWT_SECRET=<min-32-chars>               # For token signing
DEEPSEEK_API_KEY=<key>                  # AI chat features
DEEPSEEK_BASE_URL=https://api.deepseek.com
DEEPSEEK_MODEL=deepseek-chat
COVALENT_API_KEY=<key>                  # Balance/tx-history queries (has hardcoded fallback)
CORS_ALLOWED_ORIGINS=http://localhost:3000
```

Per-chain RPC overrides: `ETH_MAINNET_RPC_URL`, `BASE_MAINNET_RPC_URL`, `ARB_MAINNET_RPC_URL`, `OP_MAINNET_RPC_URL`, `BSC_MAINNET_RPC_URL`, `POLYGON_MAINNET_RPC_URL`, `ETH_SEPOLIA_RPC_URL`, `BASE_SEPOLIA_RPC_URL`.

DB pool tuning: `DB_MAX_CONNECTIONS` (default 20), `DB_MIN_CONNECTIONS` (default 5), `DB_ACQUIRE_TIMEOUT`, `DB_IDLE_TIMEOUT`, `DB_MAX_LIFETIME`.

Docker Compose maps non-standard host ports (Postgres:5433, Redis:6380, NATS:4223) to avoid conflicts with local services.

## Database

Migrations in `backend/migrations/` (010 files as of now). Auto-run on api-server startup via `sqlx::migrate!("../migrations")`. Key tables: `users`, `mpc_sessions`, `mpc_messages`, `shard_metadata`, `transactions`, `policies`, `chat_history`.

## Common Issues

### GCC Version Error (CentOS/Linux)

Error: "COMPILER BUG DETECTED" from aws-lc-sys. Fix: upgrade to GCC 11+.

### PostgreSQL Authentication

The `make local-init` target auto-configures trust auth for local connections. If auth fails manually, change `pg_hba.conf` peer -> trust and restart PostgreSQL.

## Deployment

Production deploys to ECS via GitHub Actions (`.github/workflows/deploy-ecs.yml`). The Dockerfile builds all workspace binaries; the specific binary is selected via `command:` in docker-compose.
