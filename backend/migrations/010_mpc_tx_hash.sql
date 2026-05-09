-- Add tx_hash column to mpc_sessions for tracking transaction broadcast results

ALTER TABLE mpc_sessions ADD COLUMN IF NOT EXISTS tx_hash BYTEA;

CREATE INDEX IF NOT EXISTS idx_mpc_sessions_tx_hash ON mpc_sessions(tx_hash) WHERE tx_hash IS NOT NULL;
