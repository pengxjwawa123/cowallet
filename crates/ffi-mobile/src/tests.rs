// Unit tests for ffi-mobile crate

#[cfg(test)]
mod tests {
    use crate::api::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_generate_wallet_creates_valid_address() {
        let wallet = generate_wallet().expect("Failed to generate wallet");
        
        // Verify address format: 0x + 40 hex chars
        assert!(wallet.address.starts_with("0x"), "Address should start with 0x");
        assert_eq!(wallet.address.len(), 42, "Address should be 42 chars (0x + 40 hex)");
        
        // Verify public key length: 33 bytes (compressed) or 65 bytes (uncompressed)
        assert!(
            wallet.public_key.len() == 33 || wallet.public_key.len() == 65,
            "Public key should be 33 (compressed) or 65 (uncompressed) bytes"
        );
    }

    #[test]
    #[serial]
    fn test_has_wallet_after_generation() {
        let _wallet = generate_wallet().expect("Failed to generate wallet");
        assert!(has_wallet(), "has_wallet should return true after generation");
    }

    #[test]
    #[serial]
    fn test_get_key_status_returns_valid_status() {
        let _wallet = generate_wallet().expect("Failed to generate wallet");
        let status = get_key_status();
        
        // After generate_wallet (local mode), device shard should be present
        assert!(status.has_device_shard, "Device shard should be present");
        assert!(!status.address.is_empty(), "Address should not be empty");
    }

    #[test]
    #[serial]
    fn test_clear_wallet_removes_shares() {
        let _wallet = generate_wallet().expect("Failed to generate wallet");
        assert!(has_wallet(), "Wallet should exist after generation");
        
        clear_wallet();
        assert!(!has_wallet(), "Wallet should be cleared");
    }

    #[test]
    #[serial]
    fn test_sign_hash_requires_32_bytes() {
        let _wallet = generate_wallet().expect("Failed to generate wallet");
        
        // Test with wrong length
        let short_hash = vec![0u8; 31];
        let result = sign_hash(short_hash);
        assert!(result.is_err(), "sign_hash should reject non-32-byte input");
    }

    #[test]
    #[serial]
    fn test_dkg_session_lifecycle() {
        let session_id = crate::api::dkg_session_new(0)
            .expect("Failed to create DKG session")
            .session_id;
        
        assert!(!session_id.is_empty(), "Session ID should not be empty");
        // In a full test, we would process all 3 rounds and finalize
    }
}
