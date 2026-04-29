-- cowallet initial database schema

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Users table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email TEXT UNIQUE,
    device_id TEXT NOT NULL,
    public_key BYTEA,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_device_id ON users(device_id);

-- MPC sessions
CREATE TABLE mpc_sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    session_type TEXT NOT NULL CHECK (session_type IN ('dkg', 'presign', 'sign', 'reshare')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'active', 'completed', 'failed', 'expired')),
    initiator_id UUID NOT NULL REFERENCES users(id),
    parties SMALLINT[] NOT NULL,
    threshold SMALLINT NOT NULL DEFAULT 2,
    total_parties SMALLINT NOT NULL DEFAULT 3,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '5 minutes')
);

CREATE INDEX idx_mpc_sessions_status ON mpc_sessions(status) WHERE status = 'active';
CREATE INDEX idx_mpc_sessions_initiator ON mpc_sessions(initiator_id);

-- MPC protocol messages (ephemeral, cleaned up after session completes)
CREATE TABLE mpc_messages (
    id BIGSERIAL PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES mpc_sessions(id) ON DELETE CASCADE,
    from_party SMALLINT NOT NULL,
    to_party SMALLINT NOT NULL,
    round SMALLINT NOT NULL,
    payload BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_mpc_messages_session_to ON mpc_messages(session_id, to_party, round);

-- Shard metadata (not the shard itself — just tracking)
CREATE TABLE shard_metadata (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id),
    location TEXT NOT NULL CHECK (location IN ('device', 'server', 'backup')),
    status TEXT NOT NULL DEFAULT 'healthy' CHECK (status IN ('healthy', 'needs_verification', 'compromised', 'missing')),
    party_index SMALLINT NOT NULL,
    last_used TIMESTAMPTZ,
    last_verified TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, location)
);

-- Transactions
CREATE TABLE transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id),
    chain_id BIGINT NOT NULL,
    from_addr BYTEA NOT NULL,
    to_addr BYTEA NOT NULL,
    value TEXT NOT NULL,
    token TEXT,
    tx_hash BYTEA UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'signed', 'broadcast', 'confirmed', 'failed')),
    gas_used BIGINT,
    block_number BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ
);

CREATE INDEX idx_transactions_user ON transactions(user_id, created_at DESC);
CREATE INDEX idx_transactions_status ON transactions(status) WHERE status IN ('pending', 'broadcast');

-- Policies
CREATE TABLE policies (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    rules JSONB NOT NULL,
    action JSONB NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_policies_user_enabled ON policies(user_id) WHERE enabled = true;
