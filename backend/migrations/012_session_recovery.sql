-- Add 'interrupted' status to mpc_sessions for recovery support.
-- An interrupted session can be resumed within the expiry window.

ALTER TABLE mpc_sessions DROP CONSTRAINT IF EXISTS mpc_sessions_status_check;
ALTER TABLE mpc_sessions ADD CONSTRAINT mpc_sessions_status_check
    CHECK (status IN ('pending', 'active', 'interrupted', 'completed', 'failed', 'expired'));

-- Index for finding interrupted sessions by user (recovery lookup)
CREATE INDEX IF NOT EXISTS idx_mpc_sessions_user_interrupted
    ON mpc_sessions(user_id, status, created_at DESC) WHERE status IN ('active', 'interrupted');
