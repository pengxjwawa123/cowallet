-- Indexer state tracking table
CREATE TABLE indexer_state (
    chain_id BIGINT PRIMARY KEY,
    block_number BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add token_address and log_index columns for ERC-20 transactions
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS token_address BYTEA;
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS log_index BIGINT;

-- Ensure we can uniquely identify transactions (accounting for multiple logs in same tx)
CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_tx_hash_log_index ON transactions(tx_hash, log_index);

-- Index for token address queries
CREATE INDEX IF NOT EXISTS idx_transactions_token_address ON transactions(token_address) WHERE token_address IS NOT NULL;

COMMENT ON TABLE indexer_state IS 'Tracks last processed block for each chain indexer';
