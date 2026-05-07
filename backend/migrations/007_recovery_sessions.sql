-- Recovery sessions for wallet recovery flow
-- User authenticates via email + OTP, then uses backup shard + server shard to reconstruct device shard

CREATE TABLE IF NOT EXISTS recovery_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    otp_hash BYTEA NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending', -- pending, verified, completed, expired
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    device_id VARCHAR(255)
);

CREATE INDEX idx_recovery_sessions_user_id ON recovery_sessions(user_id);
CREATE INDEX idx_recovery_sessions_status ON recovery_sessions(status);
CREATE INDEX idx_recovery_sessions_expires_at ON recovery_sessions(expires_at);

-- Cleanup expired sessions (called by worker cron)
CREATE INDEX idx_recovery_sessions_cleanup ON recovery_sessions(expires_at) WHERE status = 'pending';
