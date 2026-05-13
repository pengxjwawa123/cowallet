use alloy_primitives::{Address, B256, U256, keccak256};

/// EIP-712 typed data signing for structured messages.
///
/// Used for: ERC-20 permit, ERC-2612, governance votes, off-chain orders.

/// EIP-712 Domain separator fields.
///
/// All fields are optional; the type string is built dynamically based on which fields are present.
#[derive(Debug, Clone)]
pub struct EIP712Domain {
    pub name: Option<String>,
    pub version: Option<String>,
    pub chain_id: Option<U256>,
    pub verifying_contract: Option<Address>,
    pub salt: Option<B256>,
}

impl EIP712Domain {
    /// Create a new domain with only the required fields.
    pub fn new() -> Self {
        Self {
            name: None,
            version: None,
            chain_id: None,
            verifying_contract: None,
            salt: None,
        }
    }

    /// Set the name field.
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Set the version field.
    pub fn with_version(mut self, version: String) -> Self {
        self.version = Some(version);
        self
    }

    /// Set the chain_id field.
    pub fn with_chain_id(mut self, chain_id: U256) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Set the verifying_contract field.
    pub fn with_verifying_contract(mut self, verifying_contract: Address) -> Self {
        self.verifying_contract = Some(verifying_contract);
        self
    }

    /// Set the salt field.
    pub fn with_salt(mut self, salt: B256) -> Self {
        self.salt = Some(salt);
        self
    }
}

impl Default for EIP712Domain {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the domain separator for EIP-712.
///
/// This function:
/// 1. Builds the type string based on which fields are present
/// 2. Hashes each field according to EIP-712 rules
/// 3. Returns keccak256(typeHash || encodedFields)
pub fn domain_separator(domain: &EIP712Domain) -> B256 {
    let mut fields = Vec::new();
    let mut encoded_fields = Vec::new();

    if domain.name.is_some() {
        fields.push(("string".to_string(), "name".to_string()));
    }
    if domain.version.is_some() {
        fields.push(("string".to_string(), "version".to_string()));
    }
    if domain.chain_id.is_some() {
        fields.push(("uint256".to_string(), "chainId".to_string()));
    }
    if domain.verifying_contract.is_some() {
        fields.push(("address".to_string(), "verifyingContract".to_string()));
    }
    if domain.salt.is_some() {
        fields.push(("bytes32".to_string(), "salt".to_string()));
    }

    let type_string = encode_type("EIP712Domain", &fields);
    let type_hash = keccak256(type_string.as_bytes());

    // Encode fields in the order they appear in the type string
    if let Some(ref name) = domain.name {
        encoded_fields.push(encode_field_string(name));
    }
    if let Some(ref version) = domain.version {
        encoded_fields.push(encode_field_string(version));
    }
    if let Some(chain_id) = domain.chain_id {
        encoded_fields.push(encode_field_uint256(&chain_id));
    }
    if let Some(verifying_contract) = domain.verifying_contract {
        encoded_fields.push(encode_field_address(&verifying_contract));
    }
    if let Some(salt) = domain.salt {
        encoded_fields.push(salt);
    }

    hash_struct(&type_hash, &encoded_fields)
}

/// Build the type string for EIP-712.
///
/// Example: "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
pub fn encode_type(type_name: &str, fields: &[(String, String)]) -> String {
    let field_list = fields
        .iter()
        .map(|(ty, name)| format!("{} {}", ty, name))
        .collect::<Vec<_>>()
        .join(",");
    format!("{}({})", type_name, field_list)
}

/// Hash a struct according to EIP-712.
///
/// Computes: keccak256(typeHash || field1 || field2 || ...)
pub fn hash_struct(type_hash: &B256, encoded_fields: &[B256]) -> B256 {
    let mut buf = Vec::with_capacity(32 + encoded_fields.len() * 32);
    buf.extend_from_slice(type_hash.as_ref());
    for field in encoded_fields {
        buf.extend_from_slice(field.as_ref());
    }
    keccak256(&buf)
}

/// Encode a string field for EIP-712.
///
/// Returns: keccak256(bytes(s))
pub fn encode_field_string(s: &str) -> B256 {
    keccak256(s.as_bytes())
}

/// Encode a uint256 field for EIP-712.
///
/// Returns: 32-byte big-endian representation
pub fn encode_field_uint256(val: &U256) -> B256 {
    let mut buf = [0u8; 32];
    val.to_be_bytes_vec().iter().rev().enumerate().for_each(|(i, &b)| {
        if i < 32 {
            buf[31 - i] = b;
        }
    });
    B256::from(buf)
}

/// Encode an address field for EIP-712.
///
/// Returns: address left-padded to 32 bytes
pub fn encode_field_address(addr: &Address) -> B256 {
    let mut buf = [0u8; 32];
    buf[12..32].copy_from_slice(addr.as_ref());
    B256::from(buf)
}

/// Hash structured data according to EIP-712.
///
/// Computes: keccak256("\x19\x01" || domainSeparator || structHash)
pub fn hash_typed_data(domain_separator: &B256, struct_hash: &B256) -> B256 {
    let mut buf = Vec::with_capacity(2 + 32 + 32);
    buf.extend_from_slice(&[0x19, 0x01]);
    buf.extend_from_slice(domain_separator.as_ref());
    buf.extend_from_slice(struct_hash.as_ref());
    keccak256(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn test_encode_type() {
        let fields = vec![
            ("string".to_string(), "name".to_string()),
            ("string".to_string(), "version".to_string()),
            ("uint256".to_string(), "chainId".to_string()),
            ("address".to_string(), "verifyingContract".to_string()),
        ];
        let type_string = encode_type("EIP712Domain", &fields);
        assert_eq!(
            type_string,
            "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
        );
    }

    #[test]
    fn test_encode_field_string() {
        let s = "hello";
        let encoded = encode_field_string(s);
        // keccak256("hello") = 0x1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8
        let expected = B256::from([
            0x1c, 0x8a, 0xff, 0x95, 0x06, 0x85, 0xc2, 0xed,
            0x4b, 0xc3, 0x17, 0x4f, 0x34, 0x72, 0x28, 0x7b,
            0x56, 0xd9, 0x51, 0x7b, 0x9c, 0x94, 0x81, 0x27,
            0x31, 0x9a, 0x09, 0xa7, 0xa3, 0x6d, 0xea, 0xc8,
        ]);
        assert_eq!(encoded, expected);
    }

    #[test]
    fn test_encode_field_address() {
        let addr = address!("CcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC");
        let encoded = encode_field_address(&addr);

        // Address should be left-padded to 32 bytes (12 zeros + 20 bytes)
        let mut expected = [0u8; 32];
        expected[12..32].copy_from_slice(addr.as_ref());
        assert_eq!(encoded, B256::from(expected));
    }

    #[test]
    fn test_encode_field_uint256() {
        let val = U256::from(1);
        let encoded = encode_field_uint256(&val);

        // uint256(1) should be 32 bytes with 1 at the end
        let mut expected = [0u8; 32];
        expected[31] = 1;
        assert_eq!(encoded, B256::from(expected));

        // Test larger value
        let val = U256::from(0xdeadbeef_u64);
        let encoded = encode_field_uint256(&val);
        let mut expected = [0u8; 32];
        expected[28] = 0xde;
        expected[29] = 0xad;
        expected[30] = 0xbe;
        expected[31] = 0xef;
        assert_eq!(encoded, B256::from(expected));
    }

    #[test]
    fn test_domain_separator_computation() {
        // Test with a simple domain
        let domain = EIP712Domain::new()
            .with_name("Test".to_string())
            .with_version("1".to_string())
            .with_chain_id(U256::from(1))
            .with_verifying_contract(address!("CcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC"));

        let separator = domain_separator(&domain);

        // The separator should be deterministic and non-zero
        assert_ne!(separator, B256::ZERO);

        // Same domain should produce same separator
        let separator2 = domain_separator(&domain);
        assert_eq!(separator, separator2);
    }

    #[test]
    fn test_domain_separator_with_different_fields() {
        // Test with only name
        let domain1 = EIP712Domain::new()
            .with_name("Test".to_string());
        let separator1 = domain_separator(&domain1);

        // Test with name and version
        let domain2 = EIP712Domain::new()
            .with_name("Test".to_string())
            .with_version("1".to_string());
        let separator2 = domain_separator(&domain2);

        // Different field sets should produce different separators
        assert_ne!(separator1, separator2);
    }

    #[test]
    fn test_hash_struct() {
        let type_hash = keccak256(b"TestType(string name)");
        let name_hash = encode_field_string("Alice");

        let struct_hash = hash_struct(&type_hash, &[name_hash]);

        // Should be deterministic
        let struct_hash2 = hash_struct(&type_hash, &[name_hash]);
        assert_eq!(struct_hash, struct_hash2);
    }

    #[test]
    fn test_hash_typed_data() {
        let domain = EIP712Domain::new()
            .with_name("TestDomain".to_string())
            .with_version("1".to_string())
            .with_chain_id(U256::from(1));

        let domain_separator = domain_separator(&domain);

        // Create a simple struct hash
        let type_hash = keccak256(b"Message(string content)");
        let content_hash = encode_field_string("Hello, EIP-712!");
        let struct_hash = hash_struct(&type_hash, &[content_hash]);

        let typed_data_hash = hash_typed_data(&domain_separator, &struct_hash);

        // Should be deterministic and non-zero
        assert_ne!(typed_data_hash, B256::ZERO);

        let typed_data_hash2 = hash_typed_data(&domain_separator, &struct_hash);
        assert_eq!(typed_data_hash, typed_data_hash2);
    }

    #[test]
    fn test_hash_typed_data_with_salt() {
        // Test domain with salt field
        let salt = B256::from([0x42; 32]);
        let domain = EIP712Domain::new()
            .with_name("TestDomain".to_string())
            .with_salt(salt);

        let separator = domain_separator(&domain);
        assert_ne!(separator, B256::ZERO);
    }
}
