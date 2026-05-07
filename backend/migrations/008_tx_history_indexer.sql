-- Transaction history indexer enhancements
-- Adds native ETH tracking, address indexing, and proper constraints

-- Update indexer_state to ensure it exists (already created in 003)
-- No changes needed, table already exists with (chain_id PK, block_number, updated_at)

-- Add missing columns to transactions table if not exists
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS chain_id BIGINT NOT NULL DEFAULT 84532;
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS from_addr BYTEA;
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS to_addr BYTEA;
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS token_address BYTEA;
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS block_number BIGINT;
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS log_index BIGINT;

-- Update the unique constraint to handle both native and ERC-20 transfers
-- Drop old constraint if exists
DROP INDEX IF EXISTS idx_transactions_tx_hash_log_index;

-- Create new unique constraint that handles native ETH (log_index = NULL)
CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_tx_hash_log_idx
    ON transactions(tx_hash, COALESCE(log_index, -1));

-- Add indexes for fast history queries by address
CREATE INDEX IF NOT EXISTS idx_transactions_from_addr
    ON transactions(from_addr, block_number DESC) WHERE from_addr IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_transactions_to_addr
    ON transactions(to_addr, block_number DESC) WHERE to_addr IS NOT NULL;

-- Combined index for address queries (either from or to)
CREATE INDEX IF NOT EXISTS idx_transactions_addresses_block
    ON transactions(block_number DESC) WHERE from_addr IS NOT NULL OR to_addr IS NOT NULL;

-- Index for chain-specific queries
CREATE INDEX IF NOT EXISTS idx_transactions_chain_block
    ON transactions(chain_id, block_number DESC);

-- Index for token transfers
CREATE INDEX IF NOT EXISTS idx_transactions_token
    ON transactions(token_address, block_number DESC) WHERE token_address IS NOT NULL;

-- Add wallets table for tracking addresses (if not exists from migration 006)
CREATE TABLE IF NOT EXISTS wallets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES users(id),
    name TEXT NOT NULL,
    public_key BYTEA NOT NULL,
    eth_address BYTEA NOT NULL UNIQUE,
    chain_ids BIGINT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_wallets_eth_address ON wallets(eth_address);
CREATE INDEX IF NOT EXISTS idx_wallets_user_id ON wallets(user_id);

COMMENT ON TABLE wallets IS 'MPC wallets with tracked Ethereum addresses for indexing';
COMMENT ON INDEX idx_transactions_from_addr IS 'Fast lookup for outgoing transactions';
COMMENT ON INDEX idx_transactions_to_addr IS 'Fast lookup for incoming transactions';
