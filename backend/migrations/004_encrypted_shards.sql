-- Add encrypted shard storage to shard_metadata
ALTER TABLE shard_metadata
ADD COLUMN IF NOT EXISTS encrypted_shard BYTEA,
ADD COLUMN IF NOT EXISTS nonce BYTEA,
ADD COLUMN IF NOT EXISTS encryption_key_id TEXT DEFAULT 'default',
ADD COLUMN IF NOT EXISTS wrapped_encryption_key BYTEA;

-- Add index for faster lookup
CREATE INDEX IF NOT EXISTS idx_shard_metadata_user_location
ON shard_metadata(user_id, location);

-- Comment on columns
COMMENT ON COLUMN shard_metadata.encrypted_shard IS 'AES-GCM encrypted Shamir key share';
COMMENT ON COLUMN shard_metadata.nonce IS '12-byte AES-GCM nonce (unique per encryption)';
COMMENT ON COLUMN shard_metadata.encryption_key_id IS 'KMS key identifier used for encryption';
COMMENT ON COLUMN shard_metadata.wrapped_encryption_key IS 'Wrapped data encryption key (DEK) if using envelope encryption';
