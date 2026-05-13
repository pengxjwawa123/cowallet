use alloy_primitives::{Address, Bytes, B256, U256};
use alloy_sol_types::{sol, SolCall};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

sol! {
    /// SimpleAccountFactory.createAccount function
    function createAccount(address owner, uint256 salt) returns (address);

    /// SimpleAccount.execute function for transfers
    function execute(address dest, uint256 value, bytes calldata func) external;

    /// ERC-20 transfer function
    function transfer(address to, uint256 amount) returns (bool);
}

/// ERC-4337 UserOperation for account abstraction.
///
/// Sent to a bundler which submits it to the EntryPoint contract.
/// Enables gas sponsorship (paymaster), batched operations,
/// and on-chain social recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserOperation {
    pub sender: Address,
    pub nonce: U256,
    pub init_code: Bytes,
    pub call_data: Bytes,
    pub call_gas_limit: U256,
    pub verification_gas_limit: U256,
    pub pre_verification_gas: U256,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub paymaster_and_data: Bytes,
    pub signature: Bytes,
}

impl UserOperation {
    /// Create a new UserOperation with default gas values.
    pub fn new(sender: Address, nonce: U256, call_data: Bytes) -> Self {
        Self {
            sender,
            nonce,
            init_code: Bytes::new(),
            call_data,
            call_gas_limit: U256::from(100_000),
            verification_gas_limit: U256::from(500_000),
            pre_verification_gas: U256::from(50_000),
            max_fee_per_gas: U256::from(10_000_000_000u64), // 10 gwei
            max_priority_fee_per_gas: U256::from(1_500_000_000u64), // 1.5 gwei
            paymaster_and_data: Bytes::new(),
            signature: Bytes::new(),
        }
    }

    /// Set the signature on this UserOperation from a 65-byte ECDSA signature.
    pub fn sign(&mut self, signature: &[u8; 65]) {
        self.signature = Bytes::from(signature.to_vec());
    }

    /// ABI-encode all UserOp fields except signature for hashing per ERC-4337 spec.
    ///
    /// Per the EntryPoint contract, the pack is:
    /// abi.encode(sender, nonce, keccak256(initCode), keccak256(callData),
    ///            callGasLimit, verificationGasLimit, preVerificationGas,
    ///            maxFeePerGas, maxPriorityFeePerGas, keccak256(paymasterAndData))
    pub fn pack_for_hash(&self) -> Vec<u8> {
        use sha3::{Digest, Keccak256};

        // Each field is ABI-encoded as a 32-byte word
        let mut packed = Vec::with_capacity(320);

        // sender: left-padded to 32 bytes
        packed.extend_from_slice(&[0u8; 12]);
        packed.extend_from_slice(self.sender.as_slice());

        // nonce: 32 bytes big-endian
        packed.extend_from_slice(&self.nonce.to_be_bytes::<32>());

        // keccak256(initCode)
        packed.extend_from_slice(&Keccak256::digest(&self.init_code));

        // keccak256(callData)
        packed.extend_from_slice(&Keccak256::digest(&self.call_data));

        // callGasLimit
        packed.extend_from_slice(&self.call_gas_limit.to_be_bytes::<32>());

        // verificationGasLimit
        packed.extend_from_slice(&self.verification_gas_limit.to_be_bytes::<32>());

        // preVerificationGas
        packed.extend_from_slice(&self.pre_verification_gas.to_be_bytes::<32>());

        // maxFeePerGas
        packed.extend_from_slice(&self.max_fee_per_gas.to_be_bytes::<32>());

        // maxPriorityFeePerGas
        packed.extend_from_slice(&self.max_priority_fee_per_gas.to_be_bytes::<32>());

        // keccak256(paymasterAndData)
        packed.extend_from_slice(&Keccak256::digest(&self.paymaster_and_data));

        packed
    }

    /// Calculate the UserOp hash for signing per EIP-4337 v0.6 spec.
    ///
    /// The correct formula is:
    ///   userOpHash = keccak256(abi.encode(keccak256(pack(userOp)), entryPoint, chainId))
    pub fn hash(&self, entry_point: Address, chain_id: u64) -> B256 {
        use sha3::{Digest, Keccak256};

        // Step 1: Hash the packed UserOp
        let packed = self.pack_for_hash();
        let packed_hash = Keccak256::digest(&packed);

        // Step 2: ABI-encode (packed_hash, entryPoint, chainId)
        // ABI encoding: 32 bytes hash + 32 bytes address (left-padded) + 32 bytes uint256
        let mut encoded = Vec::with_capacity(96);
        encoded.extend_from_slice(&packed_hash);
        encoded.extend_from_slice(&[0u8; 12]); // Pad address to 32 bytes
        encoded.extend_from_slice(entry_point.as_slice());
        encoded.extend_from_slice(&chain_id.to_be_bytes()); // u64 chain_id as bytes
        encoded.extend_from_slice(&[0u8; 24]); // Pad chain_id to 32 bytes (right-justified)

        // Reorder: chain_id should be big-endian u256
        let chain_id_bytes = U256::from(chain_id).to_be_bytes::<32>();
        encoded.truncate(64); // Keep hash + address
        encoded.extend_from_slice(&chain_id_bytes);

        // Step 3: Final keccak256
        B256::from_slice(&Keccak256::digest(&encoded))
    }

    /// Format the UserOperation as a JSON object for `eth_sendUserOperation` RPC.
    pub fn to_json_rpc(&self) -> serde_json::Value {
        serde_json::json!({
            "sender": format!("{}", self.sender),
            "nonce": format!("0x{:x}", self.nonce),
            "initCode": format!("0x{}", hex::encode(&self.init_code)),
            "callData": format!("0x{}", hex::encode(&self.call_data)),
            "callGasLimit": format!("0x{:x}", self.call_gas_limit),
            "verificationGasLimit": format!("0x{:x}", self.verification_gas_limit),
            "preVerificationGas": format!("0x{:x}", self.pre_verification_gas),
            "maxFeePerGas": format!("0x{:x}", self.max_fee_per_gas),
            "maxPriorityFeePerGas": format!("0x{:x}", self.max_priority_fee_per_gas),
            "paymasterAndData": format!("0x{}", hex::encode(&self.paymaster_and_data)),
            "signature": format!("0x{}", hex::encode(&self.signature)),
        })
    }

    /// Submit this signed UserOperation to a bundler via `eth_sendUserOperation`.
    ///
    /// Returns the userOpHash on success.
    pub async fn submit_to_bundler(
        &self,
        client: &Client,
        bundler_url: &str,
        entry_point: Address,
    ) -> Result<B256, String> {
        let rpc_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_sendUserOperation",
            "params": [self.to_json_rpc(), format!("{}", entry_point)]
        });

        let response = client
            .post(bundler_url)
            .json(&rpc_body)
            .send()
            .await
            .map_err(|e| format!("bundler request failed: {}", e))?;

        let rpc_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("invalid bundler response: {}", e))?;

        if let Some(error) = rpc_response.get("error") {
            let msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown bundler error");
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
            return Err(format!("bundler rejected (code {}): {}", code, msg));
        }

        let result = rpc_response
            .get("result")
            .and_then(|r| r.as_str())
            .ok_or_else(|| "no result in bundler response".to_string())?;

        // Parse the hex hash
        let hash_str = result.strip_prefix("0x").unwrap_or(result);
        let hash_bytes = hex::decode(hash_str)
            .map_err(|e| format!("invalid hash in bundler response: {}", e))?;

        if hash_bytes.len() != 32 {
            return Err(format!(
                "unexpected hash length from bundler: {} bytes",
                hash_bytes.len()
            ));
        }

        Ok(B256::from_slice(&hash_bytes))
    }
}

/// Build a UserOperation for a simple ETH/token transfer.
pub fn build_transfer_userop(
    sender: Address,
    nonce: U256,
    to: Address,
    value: U256,
    token: Option<Address>,
) -> Result<UserOperation, UserOpError> {
    let call_data = match token {
        None => {
            // Native ETH transfer: encode execute(to, value, [])
            let execute_call = executeCall {
                dest: to,
                value,
                func: Bytes::new(),
            };
            execute_call.abi_encode().into()
        }
        Some(token_addr) => {
            // ERC-20 transfer: encode execute(TokenContract, 0, transferCalldata)
            let transfer_call = transferCall {
                to,
                amount: value,
            };
            let transfer_calldata = transfer_call.abi_encode();

            let execute_call = executeCall {
                dest: token_addr,
                value: U256::ZERO,
                func: transfer_calldata.into(),
            };
            execute_call.abi_encode().into()
        }
    };

    Ok(UserOperation::new(sender, nonce, call_data))
}

/// Build init_code for deploying a new smart account using SimpleAccountFactory.
pub fn build_account_init_code(
    factory_address: Address,
    owner_pubkey: Address,
    salt: U256,
) -> Result<Bytes, UserOpError> {
    // Encode createAccount call
    let create_account_call = createAccountCall {
        owner: owner_pubkey,
        salt,
    };
    let calldata = create_account_call.abi_encode();

    // init_code = factory_address + calldata
    let mut init_code = Vec::with_capacity(20 + calldata.len());
    init_code.extend_from_slice(factory_address.as_slice());
    init_code.extend_from_slice(&calldata);

    Ok(Bytes::from(init_code))
}

/// JSON-RPC request for eth_sendUserOperation
#[derive(Debug, Serialize)]
struct SendUserOperationRequest<'a> {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: (&'a UserOperation, Address),
}

/// JSON-RPC response
#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    pub id: u64,
    pub result: Option<T>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Submit a signed UserOperation to a bundler via eth_sendUserOperation.
pub async fn submit_to_bundler(
    userop: &UserOperation,
    entry_point: Address,
    bundler_url: &str,
) -> Result<String, UserOpError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| UserOpError::NetworkError(e.to_string()))?;

    let request = SendUserOperationRequest {
        jsonrpc: "2.0",
        id: 1,
        method: "eth_sendUserOperation",
        params: (userop, entry_point),
    };

    let response = client
        .post(bundler_url)
        .json(&request)
        .send()
        .await
        .map_err(|e| UserOpError::NetworkError(e.to_string()))?;

    let rpc_response: JsonRpcResponse<String> = response
        .json()
        .await
        .map_err(|e| UserOpError::NetworkError(e.to_string()))?;

    if let Some(error) = rpc_response.error {
        return Err(UserOpError::BundlerRejected(format!(
            "code {}: {}",
            error.code, error.message
        )));
    }

    rpc_response
        .result
        .ok_or_else(|| UserOpError::BundlerRejected("no result in response".into()))
}

/// Request paymaster sponsorship for a UserOperation.
///
/// Calls the paymaster's JSON-RPC endpoint (pm_sponsorUserOperation) which returns
/// paymasterAndData. On success, updates the UserOperation with the sponsorship data.
pub async fn request_paymaster_sponsorship(
    userop: &mut UserOperation,
    paymaster_url: &str,
    entry_point: Address,
    chain_id: u64,
) -> Result<(), UserOpError> {
    #[derive(Debug, Serialize)]
    struct PaymasterRequest<'a> {
        jsonrpc: &'static str,
        id: u64,
        method: &'static str,
        params: (&'a UserOperation, Address, String),
    }

    #[derive(Debug, Deserialize)]
    struct PaymasterResponse {
        #[serde(rename = "paymasterAndData")]
        paymaster_and_data: String,
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| UserOpError::NetworkError(e.to_string()))?;

    let request = PaymasterRequest {
        jsonrpc: "2.0",
        id: 1,
        method: "pm_sponsorUserOperation",
        params: (userop, entry_point, format!("0x{:x}", chain_id)),
    };

    let response = client
        .post(paymaster_url)
        .json(&request)
        .send()
        .await
        .map_err(|e| UserOpError::PaymasterError(format!("request failed: {}", e)))?;

    let rpc_response: JsonRpcResponse<PaymasterResponse> = response
        .json()
        .await
        .map_err(|e| UserOpError::PaymasterError(format!("invalid response: {}", e)))?;

    if let Some(error) = rpc_response.error {
        return Err(UserOpError::PaymasterError(format!(
            "code {}: {}",
            error.code, error.message
        )));
    }

    let paymaster_data = rpc_response
        .result
        .ok_or_else(|| UserOpError::PaymasterError("no result in response".into()))?;

    // Parse hex string and set paymaster_and_data
    let hex_str = paymaster_data
        .paymaster_and_data
        .strip_prefix("0x")
        .unwrap_or(&paymaster_data.paymaster_and_data);
    let bytes = hex::decode(hex_str)
        .map_err(|e| UserOpError::PaymasterError(format!("invalid hex: {}", e)))?;

    userop.paymaster_and_data = Bytes::from(bytes);
    Ok(())
}

/// Estimate UserOperation gas using eth_estimateUserOperationGas.
pub async fn estimate_userop_gas(
    userop: &UserOperation,
    entry_point: Address,
    bundler_url: &str,
) -> Result<(U256, U256, U256), UserOpError> {
    #[derive(Debug, Serialize)]
    struct EstimateGasRequest<'a> {
        jsonrpc: &'static str,
        id: u64,
        method: &'static str,
        params: (&'a UserOperation, Address),
    }

    #[derive(Debug, Deserialize)]
    struct GasEstimate {
        #[serde(rename = "preVerificationGas")]
        pre_verification_gas: Option<U256>,
        #[serde(rename = "verificationGasLimit")]
        verification_gas_limit: Option<U256>,
        #[serde(rename = "callGasLimit")]
        call_gas_limit: Option<U256>,
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| UserOpError::NetworkError(e.to_string()))?;

    let request = EstimateGasRequest {
        jsonrpc: "2.0",
        id: 1,
        method: "eth_estimateUserOperationGas",
        params: (userop, entry_point),
    };

    let response = client
        .post(bundler_url)
        .json(&request)
        .send()
        .await
        .map_err(|e| UserOpError::NetworkError(e.to_string()))?;

    let rpc_response: JsonRpcResponse<GasEstimate> = response
        .json()
        .await
        .map_err(|e| UserOpError::NetworkError(e.to_string()))?;

    if let Some(error) = rpc_response.error {
        return Err(UserOpError::BundlerRejected(format!(
            "code {}: {}",
            error.code, error.message
        )));
    }

    let estimate = rpc_response
        .result
        .ok_or_else(|| UserOpError::BundlerRejected("no result in response".into()))?;

    Ok((
        estimate.pre_verification_gas.unwrap_or(U256::from(50_000)),
        estimate.verification_gas_limit.unwrap_or(U256::from(500_000)),
        estimate.call_gas_limit.unwrap_or(U256::from(100_000)),
    ))
}

#[derive(Debug, thiserror::Error)]
pub enum UserOpError {
    #[error("not yet implemented")]
    NotImplemented,

    #[error("bundler rejected: {0}")]
    BundlerRejected(String),

    #[error("paymaster error: {0}")]
    PaymasterError(String),

    #[error("account not deployed")]
    AccountNotDeployed,

    #[error("network error: {0}")]
    NetworkError(String),

    #[error("encoding error: {0}")]
    EncodingError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn test_userop_new() {
        let sender = address!("0x1234567890123456789012345678901234567890");
        let nonce = U256::from(0);
        let call_data = Bytes::from(vec![0x01, 0x02, 0x03]);

        let userop = UserOperation::new(sender, nonce, call_data.clone());

        assert_eq!(userop.sender, sender);
        assert_eq!(userop.nonce, nonce);
        assert_eq!(userop.call_data, call_data);
        assert!(!userop.call_gas_limit.is_zero());
    }

    #[test]
    fn test_build_native_transfer_userop() {
        let sender = address!("0x1234567890123456789012345678901234567890");
        let to = address!("0x0987654321098765432109876543210987654321");
        let value = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
        let nonce = U256::from(0);

        let userop = build_transfer_userop(sender, nonce, to, value, None).unwrap();

        assert_eq!(userop.sender, sender);
        assert_eq!(userop.nonce, nonce);
        assert!(!userop.call_data.is_empty());
        // First 4 bytes should be execute() function selector
        assert_eq!(&userop.call_data[0..4], &[0xb6, 0x1d, 0x27, 0xf6]);
    }

    #[test]
    fn test_build_erc20_transfer_userop() {
        let sender = address!("0x1234567890123456789012345678901234567890");
        let token = address!("0xA0b86a33d6fD033C1e6A973C3070349c66968360"); // USDC
        let to = address!("0x0987654321098765432109876543210987654321");
        let amount = U256::from(1_000_000u128); // 1 USDC (6 decimals)
        let nonce = U256::from(0);

        let userop = build_transfer_userop(sender, nonce, to, amount, Some(token)).unwrap();

        assert_eq!(userop.sender, sender);
        assert_eq!(userop.nonce, nonce);
        assert!(!userop.call_data.is_empty());
    }

    #[test]
    fn test_build_account_init_code() {
        let factory = address!("0x9406Cc6185a346906296840746125a0E44976452");
        let owner = address!("0x1234567890123456789012345678901234567890");
        let salt = U256::from(12345);

        let init_code = build_account_init_code(factory, owner, salt).unwrap();

        // First 20 bytes should be factory address
        assert_eq!(&init_code[0..20], factory.as_slice());
        // Next 4 bytes should be createAccount() function selector
        assert_eq!(&init_code[20..24], &[0x5f, 0xbf, 0xb9, 0xcf]);
        // Total length should be 20 (address) + 4 (selector) + 64 (two args) = 88
        assert_eq!(init_code.len(), 88);
    }

    #[test]
    fn test_userop_hash() {
        let sender = address!("0x1234567890123456789012345678901234567890");
        let entry_point = address!("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789");
        let nonce = U256::from(0);
        let call_data = Bytes::from(vec![0x01, 0x02, 0x03]);

        let userop = UserOperation::new(sender, nonce, call_data);
        let hash = userop.hash(entry_point, 1);

        assert_eq!(hash.as_slice().len(), 32);
    }
}
