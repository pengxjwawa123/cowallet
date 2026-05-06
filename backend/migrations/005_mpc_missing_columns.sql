-- Add missing columns for MPC functionality
ALTER TABLE mpc_messages ADD COLUMN IF NOT EXISTS verified BOOLEAN NOT NULL DEFAULT false;
