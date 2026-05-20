-- Add brute-force protection: track OTP verification attempts per recovery session
ALTER TABLE recovery_sessions ADD COLUMN IF NOT EXISTS attempts INTEGER NOT NULL DEFAULT 0;
