-- Add missing columns for MPC functionality
ALTER TABLE mpc_sessions ADD COLUMN IF NOT EXISTS last_activity TIMESTAMPTZ;
ALTER TABLE mpc_messages ADD COLUMN IF NOT EXISTS verified BOOLEAN NOT NULL DEFAULT false;
