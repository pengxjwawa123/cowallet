-- Audit logs table for sensitive operations
CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id),
    action TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ip_address TEXT,
    user_agent TEXT,
    device_attestation TEXT,
    result TEXT NOT NULL CHECK (result IN ('success', 'failed', 'denied', 'pending')),
    duration_ms INTEGER,
    details JSONB
);

-- Index for fast user history queries
CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id, timestamp DESC);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_timestamp ON audit_logs(timestamp DESC);

-- Add comment for documentation
COMMENT ON TABLE audit_logs IS 'Audit trail for all sensitive operations in CoWallet';

-- JWT Token blacklist for logout and revocation
CREATE TABLE jwt_blacklist (
    token_id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reason TEXT
);

CREATE INDEX idx_jwt_blacklist_expires ON jwt_blacklist(expires_at);
CREATE INDEX idx_jwt_blacklist_user ON jwt_blacklist(user_id);

-- Auto-cleanup function for expired tokens
CREATE OR REPLACE FUNCTION cleanup_expired_jwt()
RETURNS TRIGGER AS $$
BEGIN
    DELETE FROM jwt_blacklist WHERE expires_at < NOW();
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Clean old tokens daily via trigger
CREATE TRIGGER trigger_cleanup_jwt
AFTER INSERT ON jwt_blacklist
EXECUTE FUNCTION cleanup_expired_jwt();
