-- Multi-wallet support: wallets table + wallet_id foreign keys
-- Presignature storage for offline pre-computation
-- WebSocket session tracking

-- Wallets table: one user can have multiple wallets
CREATE TABLE wallets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL DEFAULT 'Default Wallet',
    public_key BYTEA NOT NULL,
    eth_address BYTEA NOT NULL,
    chain_ids BIGINT[] NOT NULL DEFAULT '{84532}',
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived', 'compromised')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_reshare TIMESTAMPTZ,
    reshare_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_wallets_user ON wallets(user_id);
CREATE INDEX idx_wallets_eth_address ON wallets(eth_address);
CREATE UNIQUE INDEX idx_wallets_user_pubkey ON wallets(user_id, public_key);

-- Add wallet_id to shard_metadata (nullable for migration, then backfill)
ALTER TABLE shard_metadata ADD COLUMN IF NOT EXISTS wallet_id UUID REFERENCES wallets(id);
ALTER TABLE shard_metadata DROP CONSTRAINT IF EXISTS shard_metadata_user_id_location_key;
DROP INDEX IF EXISTS shard_metadata_user_id_location_key;
CREATE UNIQUE INDEX idx_shard_unique_wallet_location
    ON shard_metadata(user_id, wallet_id, location) WHERE wallet_id IS NOT NULL;

-- Add wallet_id to mpc_sessions
ALTER TABLE mpc_sessions ADD COLUMN IF NOT EXISTS wallet_id UUID REFERENCES wallets(id);

-- Presignatures table: store pre-computed signing material
CREATE TABLE presignatures (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wallet_id UUID NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id),
    presig_data BYTEA NOT NULL,
    status TEXT NOT NULL DEFAULT 'available' CHECK (status IN ('available', 'reserved', 'consumed', 'expired')),
    reserved_by UUID REFERENCES mpc_sessions(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '24 hours'),
    consumed_at TIMESTAMPTZ
);

CREATE INDEX idx_presignatures_wallet_status ON presignatures(wallet_id, status) WHERE status = 'available';
CREATE INDEX idx_presignatures_expires ON presignatures(expires_at) WHERE status = 'available';

-- Add wallet_id to transactions for multi-wallet tracking
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS wallet_id UUID REFERENCES wallets(id);
