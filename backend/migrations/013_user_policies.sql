-- Per-user transfer limit settings for the policy engine
CREATE TABLE user_policies (
    user_id UUID PRIMARY KEY REFERENCES users(id),
    single_limit_usd DOUBLE PRECISION NOT NULL DEFAULT 500.0,
    daily_limit_usd DOUBLE PRECISION NOT NULL DEFAULT 2000.0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
