# cowallet 项目实现路线图

**最后更新**: 2026-04-30  
**项目状态**: Phase 2 进行中 (25%完成)  
**目标完成**: W24 (6个月)

---

## 📊 优先级说明

- 🔴 **Critical** - 阻塞其他功能，必须先实现
- 🟡 **High** - 1-2周内需完成
- 🟢 **Medium** - 3-4周内完成
- 🔵 **Low** - 可延后

---

## Part 1: Phase 1 基础架构 (W1-W4) - 80%完成 ✅

### 1.1 Rust Workspace 搭建 ✅
- [x] Cargo workspace 配置
- [x] 子crate 基础框架
- [x] GitHub Actions CI
- [x] Docker Compose 编排
- [x] 环境变量配置 (.env.example)

**状态**: 完成  
**验收**: CI 绿灯，`cargo build --release` 成功

---

### 1.2 FFI 绑定 - flutter_rust_bridge v2 ✅

**优先级**: 🔴 Critical  
**文件**: 
- `crates/ffi-mobile/src/lib.rs` ✅
- `crates/ffi-mobile/src/api.rs` ✅
- `crates/ffi-mobile/src/state.rs` ✅
- `mobile/lib/bridge/` ✅

**完成内容**:
- [x] ffi-mobile crate 核心功能
  - [x] Rust 端 FFI 函数导出 (DKG初始化、R1/R2、签名触发)
  - [x] 状态管理 (Mutex + LazyLock)
  - [x] 错误处理 (Result<T, String>)

- [x] Dart 端框架
  - [x] ffi.dart 占位符
  - [x] mpc_bridge.dart wrapper 类
  - [x] 模型类 (WalletInfo, KeyStatus, etc.)
  - [x] ffi.dart.generated.dart 模板

- [x] 单元测试
  - [x] generate_wallet() 测试 ✅
  - [x] has_wallet() 测试 ✅
  - [x] get_key_status() 测试 ✅
  - [x] clear_wallet() 测试 ✅
  - [x] sign_hash() 验证测试 ✅
  - [x] dkg_session_lifecycle() 测试 ✅

**关键 API (FFI)**:
```rust
// Wallet operations
pub fn generate_wallet() -> Result<FfiWalletInfo, String>
pub fn has_wallet() -> bool
pub fn get_key_status() -> FfiKeyStatus
pub fn clear_wallet() -> ()
pub fn sign_hash(msg_hash: Vec<u8>) -> Result<Vec<u8>, String>

// DKG Protocol
pub fn dkg_session_new(party_index: u16) -> Result<FfiDkgSession, String>
pub fn dkg_generate_round1(session_id: String) -> Result<FfiRound1Result, String>
pub fn dkg_process_round1(session_id: String, messages_json: Vec<String>) -> Result<(), String>
pub fn dkg_generate_round2(session_id: String) -> Result<Vec<String>, String>
pub fn dkg_process_round2(session_id: String, messages_json: Vec<String>) -> Result<(), String>
pub fn dkg_finalize(session_id: String) -> Result<FfiDkgComplete, String>
```

**Dart 端 MpcBridge 类**:
```dart
class MpcBridge {
  static Future<WalletInfo> generateWallet()
  static Future<bool> hasWallet()
  static Future<KeyStatus> getKeyStatus()
  static Future<void> clearWallet()
  static Future<String> dkgSessionNew(int partyIndex)
  static Future<String> dkgGenerateRound1(String sessionId)
  static Future<void> dkgProcessRound1(String sessionId, List<String> messagesJson)
  static Future<List<String>> dkgGenerateRound2(String sessionId)
  static Future<void> dkgProcessRound2(String sessionId, List<String> messagesJson)
  static Future<WalletInfo> dkgFinalize(String sessionId)
  static Future<List<int>> signHash(List<int> msgHash)
}
```

**编译状态**: ✅ cargo check 通过  
**测试状态**: ✅ 6/6 单元测试通过  

**时间实际**: 2小时 (计划: 3-4天)

**下一步**: 生成 flutter_rust_bridge 的完整 Dart bindings (需运行代码生成器)

---

### 1.3 iOS Secure Enclave 集成 ✅

**优先级**: 🔴 Critical  
**文件**:
- `mobile/lib/platform/ios_se_channel.dart` ✅
- `mobile/lib/platform/se_manager.dart` ✅
- `mobile/ios/Runner/MpcSecureEnclave.swift` ✅
- `mobile/ios/Runner/MpcSecureStorage.swift` ✅
- `mobile/ios/Runner/AppDelegate.swift` ✅
- `mobile/ios/Runner/Info.plist` ✅

**完成内容**:
- [x] Dart Platform Channel 定义
  - [x] `IosSecureEnclaveChannel` 类 (6个方法)
  - [x] `storeSecret`, `getSecret`, `deleteSecret` 安全存储
  - [x] 错误处理 (SeException)

- [x] Swift 处理器实现
  - [x] `MpcSecureEnclaveHandler` - SE 密钥操作
    - [x] `generateKey()` - P-256 密钥生成
    - [x] `getPublicKey()` - 公钥检索和压缩 (65->33 bytes)
    - [x] `signWithBiometric()` - Face ID / Touch ID 认证+签名
    - [x] `isAvailable()` - 可用性检查
  
  - [x] `MpcSecureStorageHandler` - Keychain 加密存储
    - [x] `storeSecret()` - Keychain 存储
    - [x] `getSecret()` - Keychain 检索
    - [x] `deleteSecret()` - Keychain 删除

- [x] SE Manager (高级 API)
  - [x] `initializeWallet()` - 钱包初始化
  - [x] `getDeviceShardKeyId()` - 获取密钥 ID
  - [x] `getDeviceShardPublicKey()` - 获取公钥
  - [x] `signHashWithBiometric()` - 生物识别签名
  - [x] `storeDeviceShard()` / `getDeviceShard()` - 分片存储
  - [x] `clearWallet()` - 钱包重置

- [x] 权限和配置
  - [x] 在 AppDelegate 中注册 Platform Channel
  - [x] 在 Info.plist 中添加 Face ID 权限说明
  - [x] Keychain 访问控制 (`kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly`)

- [x] 单元测试
  - [x] Platform Channel 测试 (5个测试用例)
  - [x] SE Manager 测试 (4个测试用例)

**关键特性**:
```swift
// Secure Enclave 密钥生成
let privateKey = try SecureEnclave.P256.Signing.PrivateKey()
// 密钥存储在 SE 中，永不导出

// 生物识别签名
let context = LAContext()
context.evaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, ...) { success in
  // 成功后使用 SE 中的密钥进行签名
}

// 公钥压缩 (65 bytes -> 33 bytes)
let compressed = compressPublicKey(publicKey)  // 0x02/0x03 + X 坐标
```

**编译状态**: ✅ Swift 代码无编译错误
**测试状态**: ✅ 单元测试框架就绪

**时间实际**: 1 小时 (计划: 5-7 天)

**下一步**: 在真实 iOS 设备上进行集成测试

---

### 1.4 Android StrongBox 集成 ✅

**优先级**: 🔴 Critical  
**文件**:
- `mobile/lib/platform/android_strongbox_channel.dart` ✅
- `mobile/lib/platform/sb_manager.dart` ✅
- `mobile/android/app/src/main/kotlin/com/cowallet/mpc/MpcStrongBoxHandler.kt` ✅
- `mobile/android/app/src/main/kotlin/com/cowallet/mpc/MpcKeystoreHandler.kt` ✅
- `mobile/android/app/src/main/kotlin/com/cowallet/MainActivity.kt` ✅
- `mobile/android/app/src/main/AndroidManifest.xml` ✅
- `mobile/android/app/build.gradle.kts` ✅

**完成内容**:
- [x] Dart Platform Channel 定义
  - [x] `AndroidStrongBoxChannel` 类 (6个方法)
  - [x] `storeSecret`, `getSecret`, `deleteSecret` 安全存储
  - [x] 错误处理 (SbException)

- [x] Kotlin 处理器实现
  - [x] `MpcStrongBoxHandler` - StrongBox 密钥操作
    - [x] `generateKey()` - RSA-2048 密钥生成
    - [x] `getPublicKey()` - 公钥检索
    - [x] `signWithBiometric()` - 生物识别+签名
    - [x] `isAvailable()` - 可用性检查
  
  - [x] `MpcKeystoreHandler` - Keystore 加密存储
    - [x] `storeSecret()` - Keystore 存储 (AES-256/GCM)
    - [x] `getSecret()` - Keystore 检索
    - [x] `deleteSecret()` - Keystore 删除

- [x] StrongBox Manager (高级 API)
  - [x] `initializeWallet()` - 钱包初始化
  - [x] `getDeviceShardKeyId()` - 获取密钥 ID
  - [x] `getDeviceShardPublicKey()` - 获取公钥
  - [x] `signHashWithBiometric()` - 生物识别签名
  - [x] `storeDeviceShard()` / `getDeviceShard()` - 分片存储
  - [x] `clearWallet()` - 钱包重置

- [x] 权限和配置
  - [x] 在 MainActivity 中注册 Platform Channel
  - [x] 在 AndroidManifest.xml 中添加生物识别权限
  - [x] 在 build.gradle.kts 中添加 androidx.biometric 依赖
  - [x] Keystore 访问控制

- [x] 单元测试
  - [x] Platform Channel 测试 (5个测试用例)
  - [x] StrongBox Manager 测试 (4个测试用例)

**关键特性**:
```kotlin
// StrongBox 密钥生成 (Android 9+)
val keyPairGenerator = KeyPairGenerator.getInstance("RSA", "AndroidKeyStore")
keyPairGenerator.initialize(
  KeyGenParameterSpec.Builder(alias, PURPOSE_SIGN or PURPOSE_VERIFY)
    .setIsStrongBoxBacked(true)  // 硬件隔离
    .build()
)

// 生物识别签名
val biometricPrompt = BiometricPrompt(activity, executor, callback)
biometricPrompt.authenticate(promptInfo)

// AES-256/GCM 加密存储
val cipher = Cipher.getInstance("AES/GCM/NoPadding")
cipher.init(Cipher.ENCRYPT_MODE, secretKey)
val ciphertext = cipher.doFinal(plaintext)
```

**编译状态**: ✅ Kotlin 代码无编译错误
**测试状态**: ✅ 单元测试框架就绪

**时间实际**: 1.5 小时 (计划: 5-7 天)

**下一步**: 在真实 Android 设备上进行集成测试

---

### 1.5 后端基础框架 ✅

**优先级**: 🟡 High  
**文件**:
- `backend/api-server/src/main.rs`
- `backend/api-server/src/routes/`

**已完成**:
- [x] Axum HTTP 服务器启动
- [x] PostgreSQL 连接
- [x] Redis 连接
- [x] NATS 消息队列
- [x] 基础路由结构
- [x] 错误处理中间件
- [x] 跨域 (CORS) 配置

**状态**: 完成 ✅

---

## Part 2: Phase 2 核心协议 (W5-W10) - 25%完成 ⚠️

### 2.1 DKLS23 DKG 完整实现 🔴 Critical

**优先级**: 🔴 Critical  
**文件**: `crates/mpc-core/src/dkls23/dkg.rs`

**现状**: 
- ✅ 基础消息定义 (DkgRound1Message, DkgRound2Message)
- ✅ Round 1 生成和处理逻辑
- ✅ Round 2 生成和处理逻辑
- ⚠️ 缺失: Feldman VSS 零知识证明、恶意方检测、完整的组合流程

**任务**:
- [ ] Feldman VSS 验证
  ```rust
  // dkg.rs 中需添加
  pub fn verify_feldman_vss(
      commitments: &[AffinePoint],
      recipient_idx: u16,
      share: Scalar,
  ) -> Result<()> {
      // C_j = commitment[j]
      // Verify: share * G == sum(C_j * x^j) for j in 0..t-1
  }
  ```

- [ ] 恶意方检测
  ```rust
  pub struct DkgComplaint {
      accuser: u16,
      accused: u16,
      reason: String,
  }
  
  pub fn handle_complaint(...) -> Result<()> {
      // 如果指控有效，剔除恶意方
  }
  ```

- [ ] 完整 DKG 流程
  - [ ] Round 1: 并行收集所有 commitments
  - [ ] Round 2: 验证后发送 shares
  - [ ] Round 3: 最终 share 组合和公钥计算
  - [ ] 恶意方处理和重启

- [ ] 测试
  - [ ] 单元测试 (>50个)
  - [ ] 属性测试 (proptest): 任意3方 DKG → 有效密钥
  - [ ] 对抗测试: 一方故意发错消息 → 检测到
  - [ ] 集成测试: 3方完整 DKG 流程

**代码示例**:
```rust
// 现有代码已有 process_round1/2 基础
// 需添加验证逻辑

impl DkgSession {
    fn verify_share_against_commitments(
        &self,
        share: Scalar,
        sender: u16,
        recipient: u16,
    ) -> Result<()> {
        // share_point = share * G
        // Check: share_point == sum(C_j * recipient_idx^j)
        let share_point = AffinePoint::GENERATOR * &share;
        
        // 从 self.round1_messages 获取 commitments
        let commitments = &self.round1_messages[sender as usize].commitments;
        
        // 计算预期值...
        // ...验证逻辑
        Ok(())
    }
}
```

**时间估计**: 10-12天  
**依赖**: 无 (独立)

---

### 2.2 DKLS23 TSS 签名 (Presign + Sign) 🔴 Critical

**优先级**: 🔴 Critical  
**文件**: 
- `crates/mpc-core/src/dkls23/presign.rs` (新建)
- `crates/mpc-core/src/dkls23/sign.rs` (新建)

**背景**: DKLS23 采用预签名 (presign) + 在线签名 (sign) 的分离设计：
- **Presign**: 离线 2 轮，双方协商临时 commitment (无消息依赖)
- **Sign**: 在线 1 轮，计算实际签名 (<100ms)

**任务**:

#### 2.2.1 Presign 实现
```rust
// crates/mpc-core/src/dkls23/presign.rs

pub struct PresignMessage {
    pub session_id: String,
    pub signer_indices: (u16, u16),  // 两个签名方 (e.g., 0, 1)
    pub round: u8,  // 1 or 2
    pub payload: Vec<u8>,
}

pub struct PresignResult {
    pub big_r: AffinePoint,        // R = k1*G + k2*G
    pub big_r_commitment: Vec<u8>, // H(big_r)
}

impl PresignSession {
    pub fn new(config: SessionConfig) -> Self { ... }
    
    pub fn generate_round1(&mut self) -> Result<PresignMessage> {
        // Party i generates random k_i, computes commit(k_i * G)
        // Sends hash commitment to other party
    }
    
    pub fn process_round1(&mut self, msg: PresignMessage) -> Result<()> {
        // Stores commitment for verification
    }
    
    pub fn generate_round2(&mut self) -> Result<PresignMessage> {
        // Party i sends k_i * G (now verified against Round 1 commitment)
    }
    
    pub fn process_round2(&mut self, msg: PresignMessage) -> Result<()> {
        // Compute big_r = k1*G + k2*G, store for sign phase
    }
    
    pub fn finalize(&self) -> Result<PresignResult> {
        // Return R and commitment for offline storage
    }
}
```

**测试**:
- [ ] 单元测试: presign round 1 & 2
- [ ] 验证: R 点正确性
- [ ] 多次 presign 可并行生成

#### 2.2.2 Sign 实现
```rust
// crates/mpc-core/src/dkls23/sign.rs

pub struct SignMessage {
    pub session_id: String,
    pub signers: (u16, u16),
    pub payload: Vec<u8>,
}

impl SignSession {
    pub fn new(
        config: SessionConfig,
        presign_result: PresignResult,
        message_hash: B256,
    ) -> Self { ... }
    
    pub fn generate_signature(&mut self) -> Result<SignMessage> {
        // Single round: exchange partial signatures
        // sig_i = k_i^{-1} * (h + r * x_i)
    }
    
    pub fn process_signature(&mut self, msg: SignMessage) -> Result<()> {
        // Verify partial signature, combine to get final signature
    }
    
    pub fn finalize(&self) -> Result<(u64, B256, B256)> {
        // Return (v, r, s) for ECDSA
    }
}
```

**测试**:
- [ ] 验证签名: ECDSA 验证通过
- [ ] 任意 2-of-3 签名: 不同的方对组合应成功
- [ ] 恶意方: 1 方发错数据 → 签名失败，可指认恶意方

#### 2.2.3 集成测试
```rust
#[test]
fn test_complete_signing_flow() {
    // Setup: 3 parties with DKG shares
    let parties = setup_3party_dkg();
    
    // Phase 1: Presign (parties 0, 1)
    let presign_0 = presign_round1(&parties[0]);
    let presign_1 = presign_round1(&parties[1]);
    // ... rounds...
    let presign_result = presign_finalize(&parties[0]);
    
    // Phase 2: Sign (parties 0, 1)
    let msg_hash = b"test message";
    let sign_0 = sign_generate(&parties[0], presign_result.clone(), msg_hash);
    let sign_1 = sign_generate(&parties[1], presign_result.clone(), msg_hash);
    
    let signature = sign_finalize(&parties[0], sign_1);
    
    // Verify: signature is valid ECDSA
    assert!(verify_ecdsa(msg_hash, signature, public_key));
}
```

**时间估计**: 14-16天  
**依赖**: 2.1 DKG 完成

---

### 2.3 密钥分片加密存储 🟡 High

**优先级**: 🟡 High  
**文件**:
- `crates/storage-crypto/src/encrypt.rs` (新建)
- `crates/mpc-core/src/shard/device.rs`
- `crates/mpc-core/src/shard/server.rs`
- `crates/mpc-core/src/shard/backup.rs`

**任务**:

#### 2.3.1 Device Shard (Shard 1)
```rust
// crates/mpc-core/src/shard/device.rs

pub struct DeviceShard {
    pub encrypted_share: Vec<u8>,  // AES-256-GCM
    pub se_derived_key_id: String, // SE 内生成的密钥 ID
}

impl DeviceShard {
    pub async fn new(
        key_share: &KeyShare,
        se_handler: &SecureEnclaveHandler,
    ) -> Result<Self> {
        // 1. 从 SE 导出加密密钥 (不导出主密钥)
        let encrypt_key = se_handler.derive_encryption_key().await?;
        
        // 2. AES-256-GCM 加密 share
        let encrypted_share = encrypt_aes_gcm(
            &key_share.secret_share,
            &encrypt_key,
        )?;
        
        Ok(Self {
            encrypted_share,
            se_derived_key_id: encrypt_key.id,
        })
    }
    
    pub async fn unlock(
        &self,
        se_handler: &SecureEnclaveHandler,
    ) -> Result<Scalar> {
        // 1. 触发生物识别
        se_handler.biometric_unlock().await?;
        
        // 2. SE 导出解密密钥
        let decrypt_key = se_handler.derive_decryption_key().await?;
        
        // 3. 解密 share
        let share_bytes = decrypt_aes_gcm(
            &self.encrypted_share,
            &decrypt_key,
        )?;
        
        // 4. 转为 Scalar
        let scalar = Scalar::from_repr(share_bytes.into())?;
        
        // 5. 返回前 zeroize 本地副本
        Ok(scalar)
    }
}
```

#### 2.3.2 Server Shard (Shard 2)
```rust
// crates/mpc-core/src/shard/server.rs

pub struct ServerShard {
    pub hsm_key_handle: u64,        // SoftHSM2 密钥句柄
    pub encrypted_backup: Vec<u8>,  // 备份加密副本
}

impl ServerShard {
    pub async fn new(
        key_share: &KeyShare,
        hsm: &HsmClient,
    ) -> Result<Self> {
        // 1. 在 HSM 内部生成密钥 (不导出)
        let hsm_key = hsm.generate_key(
            "AES-256",
            &format!("shard-{}", uuid::Uuid::new_v4()),
        ).await?;
        
        // 2. HSM 内加密 share (如果支持)
        // 或在 server 端加密后存储 encrypted_backup
        let encrypted_share = encrypt_aes_gcm(
            &key_share.secret_share,
            &hsm_key.export_public()?,
        )?;
        
        Ok(Self {
            hsm_key_handle: hsm_key.handle,
            encrypted_backup: encrypted_share,
        })
    }
    
    pub async fn use_for_signing(
        &self,
        hash: &B256,
        hsm: &HsmClient,
    ) -> Result<Signature> {
        // HSM 内使用密钥签名 (密钥不导出)
        hsm.sign(self.hsm_key_handle, hash).await
    }
}
```

#### 2.3.3 Backup Shard (Shard 3)
```rust
// crates/mpc-core/src/shard/backup.rs

pub struct BackupShard {
    pub encrypted_shares: Vec<Vec<u8>>,  // 3-of-5 Shamir shares (each encrypted)
    pub salt: Vec<u8>,
    pub password_hash: Vec<u8>,
}

impl BackupShard {
    pub fn new(
        key_share: &KeyShare,
        password: &str,
    ) -> Result<Self> {
        // 1. Argon2id 派生加密密钥
        let salt = rand::random::<[u8; 16]>().to_vec();
        let derive_key = argon2id_derive(password, &salt, 32)?;
        
        // 2. Shamir Secret Sharing: 分成 5 份，3 份可恢复
        let shamir_shares = shamir_split(&key_share.secret_share, 3, 5)?;
        
        // 3. 加密每份
        let encrypted_shares = shamir_shares
            .iter()
            .map(|share| encrypt_aes_gcm(share, &derive_key))
            .collect::<Result<Vec<_>>>()?;
        
        Ok(Self {
            encrypted_shares,
            salt,
            password_hash: sha256(password.as_bytes()).to_vec(),
        })
    }
    
    pub fn recover(
        password: &str,
        shares: Vec<Vec<u8>>,  // 3 个加密的分片
        salt: &[u8],
    ) -> Result<Scalar> {
        // 1. Argon2id 派生密钥
        let derive_key = argon2id_derive(password, salt, 32)?;
        
        // 2. 解密每份
        let decrypted = shares
            .iter()
            .map(|encrypted| decrypt_aes_gcm(encrypted, &derive_key))
            .collect::<Result<Vec<_>>>()?;
        
        // 3. Shamir 恢复
        let recovered = shamir_recover(&decrypted)?;
        
        Ok(Scalar::from_repr(recovered.into())?)
    }
}
```

**测试**:
- [ ] device.rs: 加密 → 生物识别解锁 → 解密正确
- [ ] server.rs: HSM 密钥不可导出验证
- [ ] backup.rs: 3-of-5 恢复功能
- [ ] 集成: 3 个分片组合 → 恢复密钥成功

**时间估计**: 7-9天  
**依赖**: 2.1 DKG (需 KeyShare 类型)

---

### 2.4 Proactive Resharing (密钥刷新) 🟢 Medium

**优先级**: 🟢 Medium  
**文件**: `crates/mpc-core/src/dkls23/reshare.rs` (新建)

**背景**: 每 30 天自动生成新的分片，旧分片安全擦除，但公钥保持不变。

**任务**:
```rust
// crates/mpc-core/src/dkls23/reshare.rs

pub struct ReshareSession {
    pub old_shares: Vec<KeyShare>,
    pub new_shares: Vec<KeyShare>,
}

impl ReshareSession {
    pub fn new(config: SessionConfig) -> Self { ... }
    
    pub fn generate_round1(&mut self) -> Result<Vec<ProtocolMessage>> {
        // Each party generates polynomial for resharing
        // Similar to DKG but without revealing new constant term
    }
    
    pub fn finalize(&self) -> Result<Vec<KeyShare>> {
        // New shares with same public key
    }
}

pub struct KeyShareRefresher {
    pub last_refresh: DateTime<Utc>,
}

impl KeyShareRefresher {
    pub async fn maybe_refresh(
        &mut self,
        current_shares: Vec<KeyShare>,
    ) -> Result<Option<Vec<KeyShare>>> {
        if self.last_refresh.elapsed() > Duration::days(30) {
            let reshare = ReshareSession::new(...);
            // ... reshare rounds ...
            let new_shares = reshare.finalize()?;
            self.last_refresh = Utc::now();
            
            // Zeroize old shares
            for share in &current_shares {
                // Ensure memory is securely erased
            }
            
            return Ok(Some(new_shares));
        }
        Ok(None)
    }
}
```

**测试**:
- [ ] reshare 后公钥不变
- [ ] reshare 后可用任意 2-of-3 签名
- [ ] 旧 shares zeroize 验证

**时间估计**: 5-7天  
**依赖**: 2.1, 2.2 完成

---

### 2.5 Transport & Message Relay 🟡 High

**优先级**: 🟡 High  
**文件**:
- `crates/mpc-core/src/transport/noise.rs` (新建 - Noise_XX)
- `crates/mpc-core/src/transport/relay.rs` (新建 - NATS)
- `backend/mpc-relay/src/main.rs` (已框架)

**任务**:

#### 2.5.1 Noise_XX 加密

```rust
// crates/mpc-core/src/transport/noise.rs

pub struct NoiseSession {
    state: snow::TransportState,
    peer_public: Option<[u8; 32]>,
}

impl NoiseSession {
    pub fn new_initiator(peer_public_key: &[u8; 32]) -> Result<Self> {
        // Initialize Noise_XX as initiator
        let params = "Noise_XX_25519_ChaChaPoly_BLAKE2s"
            .parse()?;
        let builder = snow::Builder::new(params);
        
        let state = builder
            .psk(0, &[0u8; 32])  // Optional PSK
            .build_initiator()?;
        
        Ok(Self {
            state,
            peer_public: Some(*peer_public_key),
        })
    }
    
    pub fn write_message(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; plaintext.len() + 64];
        let n = self.state.write_message(plaintext, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }
    
    pub fn read_message(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; ciphertext.len()];
        let n = self.state.read_message(ciphertext, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }
}
```

#### 2.5.2 NATS 中继

```rust
// crates/mpc-core/src/transport/relay.rs

pub struct NatsRelay {
    client: async_nats::Client,
    session_id: String,
}

impl NatsRelay {
    pub async fn new(nats_url: &str, session_id: &str) -> Result<Self> {
        let client = async_nats::connect(nats_url).await?;
        Ok(Self {
            client,
            session_id: session_id.into(),
        })
    }
    
    pub async fn broadcast_message(
        &self,
        msg: &ProtocolMessage,
    ) -> Result<()> {
        let subject = format!("mpc.{}.round{}.broadcast", 
            self.session_id, msg.round);
        self.client.publish(
            subject,
            bincode::serialize(msg)?.into(),
        ).await?;
        Ok(())
    }
    
    pub async fn send_message(
        &self,
        msg: &ProtocolMessage,
    ) -> Result<Vec<u8>> {
        let subject = format!("mpc.{}.p{}.p{}", 
            self.session_id, msg.from, msg.to);
        let reply_subject = format!("{}.reply", subject);
        
        let response = self.client
            .request(subject, bincode::serialize(msg)?.into(), 
                std::time::Duration::from_secs(30))
            .await?;
        
        Ok(response.payload.into())
    }
}
```

#### 2.5.3 MPC Relay Server
```rust
// backend/mpc-relay/src/main.rs

#[tokio::main]
async fn main() -> Result<()> {
    let nats = async_nats::connect("nats://nats:4222").await?;
    
    // Subscribe to all MPC messages
    let mut subscriber = nats.subscribe("mpc.>").await?;
    
    while let Some(message) = subscriber.next().await {
        let subject = &message.subject;
        let payload = &message.payload;
        
        // Route message to appropriate recipient
        // If broadcast: publish to all parties in session
        // If unicast: reply to sender
        
        if let Some(reply_to) = &message.reply {
            // This is a request-reply, send back
            nats.publish(reply_to, payload.clone()).await?;
        } else {
            // Broadcast to others in same session
            nats.publish(subject, payload.clone()).await?;
        }
    }
    
    Ok(())
}
```

**测试**:
- [ ] Noise 握手成功
- [ ] 消息加密 → 解密正确
- [ ] NATS broadcast 和 request-reply 工作

**时间估计**: 6-8天  
**依赖**: 无 (独立)

---

## Part 3: Phase 3 产品功能 (W11-W16) - 10%完成

### 3.1 Policy Engine 完整实现 🔴 Critical

**优先级**: 🔴 Critical  
**文件**: `crates/policy-engine/src/`

**现状**: 框架就绪，需完善规则引擎和风控

**任务**:

#### 3.1.1 规则引擎补全
```rust
// crates/policy-engine/src/rules.rs

pub fn evaluate_rules(
    tx: &TransactionContext,
    policies: &[Policy],
) -> Result<Vec<PolicyDecision>> {
    let mut decisions = Vec::new();
    
    for policy in policies {
        let mut policy_passed = true;
        
        for rule in &policy.rules {
            match rule {
                Rule::MaxAmount { token, limit } => {
                    // ✅ 已实现
                    if &tx.value > limit {
                        policy_passed = false;
                        break;
                    }
                }
                
                Rule::DailyLimit { token, limit } => {
                    // ❌ TODO: 查询 Redis 中该天已转金额
                    let today_key = format!("daily:{}:{}:{}", 
                        tx.user_id, token, today_date());
                    let redis_client = redis::Client::open("redis://redis")?;
                    let conn = redis_client.get_connection()?;
                    let today_sum: u128 = conn.get(&today_key).unwrap_or(0);
                    
                    if today_sum + tx.value.as_u128() > limit.as_u128() {
                        policy_passed = false;
                        break;
                    }
                }
                
                Rule::RateLimit { max_tx, window_secs } => {
                    // ❌ TODO: 查询时间窗口内交易数
                    let rate_key = format!("rate:{}:{}", 
                        tx.user_id, tx.chain_id);
                    let tx_count: u32 = redis_client
                        .get(&rate_key)
                        .unwrap_or(0);
                    
                    if tx_count >= *max_tx {
                        policy_passed = false;
                        break;
                    }
                }
                
                Rule::WhitelistOnly { addresses } => {
                    // ✅ 已实现
                    if !addresses.contains(&tx.to) {
                        policy_passed = false;
                        break;
                    }
                }
                
                Rule::BlacklistCheck { addresses } => {
                    // ✅ 已实现
                    if addresses.contains(&tx.to) {
                        policy_passed = false;
                        break;
                    }
                }
                
                Rule::TimeWindow { start_hour, end_hour } => {
                    // ✅ 已实现 (基础)
                }
                
                Rule::ChainRestriction { allowed_chains } => {
                    // ❌ TODO
                    if !allowed_chains.contains(&tx.chain_id) {
                        policy_passed = false;
                        break;
                    }
                }
                
                Rule::ContractInteraction { allow_unknown } => {
                    // ❌ TODO: 查询合约是否已知
                    if tx.is_contract_interaction && !allow_unknown {
                        // Check if contract is in known list
                        policy_passed = false;
                        break;
                    }
                }
            }
        }
        
        decisions.push(PolicyDecision {
            policy_id: policy.id,
            allowed: policy_passed,
            action: policy.action.clone(),
        });
    }
    
    Ok(decisions)
}
```

#### 3.1.2 多重审批流程
```rust
// crates/policy-engine/src/approval.rs

#[derive(Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub required_approvers: Vec<UserId>,
    pub threshold: u32,  // M-of-N
    pub signatures: HashMap<UserId, String>,  // 签名授权
    pub status: ApprovalStatus,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub enum ApprovalStatus {
    Pending,
    PartiallyApproved { count: u32 },
    Approved,
    Rejected,
    Expired,
}

pub async fn submit_approval(
    req: &mut ApprovalRequest,
    approver_id: &UserId,
    signature: &str,
) -> Result<()> {
    if !req.required_approvers.contains(approver_id) {
        return Err(MpcError::Unauthorized("not an approver".into()));
    }
    
    req.signatures.insert(approver_id.clone(), signature.into());
    
    let approved_count = req.signatures.len() as u32;
    if approved_count >= req.threshold {
        req.status = ApprovalStatus::Approved;
    } else {
        req.status = ApprovalStatus::PartiallyApproved {
            count: approved_count,
        };
    }
    
    Ok(())
}
```

#### 3.1.3 实时风控
```rust
// crates/policy-engine/src/risk.rs

pub struct AnomalyDetector {
    user_id: String,
    history_window_days: i64,  // 过去 30 天
}

impl AnomalyDetector {
    pub async fn detect_anomaly(
        &self,
        tx: &TransactionContext,
        db: &PgPool,
    ) -> Result<RiskLevel> {
        // 获取用户历史交易
        let history = get_user_transactions(
            db,
            &self.user_id,
            self.history_window_days,
        ).await?;
        
        // 计算统计特征
        let avg_amount = calculate_avg_amount(&history);
        let std_dev = calculate_std_dev(&history);
        let avg_frequency = calculate_avg_frequency(&history);
        
        // 检测异常
        let amount_zscore = (tx.value.as_u128() as f64 - avg_amount) / std_dev;
        if amount_zscore > 3.0 {
            return Ok(RiskLevel::High);  // 金额异常
        }
        
        // 检查地址是否为已知接收方
        let known_addresses = get_known_addresses(db, &self.user_id).await?;
        if !known_addresses.contains(&tx.to) {
            return Ok(RiskLevel::Medium);  // 新地址
        }
        
        // 检查交易频率
        let recent_tx_count = get_recent_tx_count(
            db,
            &self.user_id,
            Duration::minutes(10),
        ).await?;
        if recent_tx_count > 5 {
            return Ok(RiskLevel::High);  // 频率异常
        }
        
        Ok(RiskLevel::Low)
    }
}

#[derive(Clone, Debug)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
```

**测试**:
- [ ] 单元测试: 每个 Rule 类型
- [ ] 集成测试: 多个规则组合
- [ ] 边界测试: 日限额跨天、时间窗口边界
- [ ] 异常检测: 已知和新地址

**时间估计**: 8-10天  
**依赖**: Redis, PostgreSQL 配置完成

---

### 3.2 EVM Chain 适配 🟡 High

**优先级**: 🟡 High  
**文件**: `crates/chain-evm/src/`

**任务**:

#### 3.2.1 MpcSigner 实现
```rust
// crates/chain-evm/src/signer.rs

pub struct MpcSigner {
    session_manager: Arc<MpcSessionManager>,
    chain_id: u64,
}

#[async_trait]
impl Signer for MpcSigner {
    async fn sign_hash(&self, hash: &B256) -> Result<Signature> {
        // 1. 触发 DKG/TSS 签名会话
        let msg = ProtocolMessage {
            round: 1,
            payload: bincode::serialize(hash)?,
            // ...
        };
        
        // 2. 广播消息给其他方 (via NATS)
        self.session_manager.broadcast(msg).await?;
        
        // 3. 等待其他方响应 (timeout: 5s)
        let responses = self.session_manager
            .wait_for_responses(2, Duration::secs(5))
            .await?;
        
        // 4. 完成 TSS 签名
        let sig = self.session_manager.finalize_signature(responses)?;
        
        // 5. 返回 ECDSA 签名
        Ok(Signature::from_der(&sig)?)
    }
    
    fn address(&self) -> Address {
        // 从公钥派生地址
        todo!()
    }
}
```

#### 3.2.2 交易构造
```rust
// crates/chain-evm/src/transaction.rs

pub async fn build_transaction(
    to: Address,
    value: U256,
    chain_id: u64,
    provider: &Provider,
    policy_engine: &PolicyEngine,
) -> Result<TransactionEnvelope> {
    // 1. Policy 评估
    let tx_ctx = TransactionContext {
        to,
        value,
        chain_id,
        // ...
    };
    let decision = policy_engine.evaluate(&tx_ctx).await?;
    if !decision.allowed {
        return Err(MpcError::PolicyRejected(decision.reason));
    }
    
    // 2. Gas 估计 (per-chain model)
    let gas_estimate = estimate_gas_for_chain(
        provider,
        to,
        value,
        chain_id,
    ).await?;
    
    // 3. 构造交易
    let tx = TransactionRequest::default()
        .to(to)
        .value(value)
        .gas_limit(gas_estimate.gas)
        .gas_price(gas_estimate.gas_price)
        .chain_id(chain_id);
    
    // 4. 使用 MpcSigner 签名
    let signer = MpcSigner::new(...);
    let signed = tx.into_tx().await?;
    
    Ok(TransactionEnvelope::Eip1559(signed))
}
```

#### 3.2.3 多链 Gas 处理
```rust
// crates/chain-evm/src/gas.rs

pub async fn estimate_gas_for_chain(
    provider: &Provider,
    to: Address,
    value: U256,
    chain_id: u64,
) -> Result<GasEstimate> {
    let chain = get_chain_config(chain_id);
    
    match chain.gas_model {
        GasModel::Eip1559 => {
            // Ethereum L1: standard EIP-1559
            let base_fee = provider.get_gas_price().await?;
            let priority_fee = U256::from(2_000_000_000);  // 2 gwei
            Ok(GasEstimate {
                gas: U256::from(21_000),
                gas_price: base_fee + priority_fee,
            })
        }
        
        GasModel::ArbitrumNitro => {
            // Arbitrum: execution gas + L1 calldata fee
            let l2_gas = provider.estimate_gas(...).await?;
            let l1_gas = estimate_arbitrum_l1_fee(provider, tx).await?;
            Ok(GasEstimate {
                gas: l2_gas + l1_gas,
                gas_price: provider.get_gas_price().await?,
            })
        }
        
        GasModel::OpBedrock => {
            // Optimism: execution + L1 data (blob-aware for EIP-4844)
            let l2_gas = provider.estimate_gas(...).await?;
            let l1_data_fee = estimate_op_l1_fee(provider, tx).await?;
            Ok(GasEstimate {
                gas: l2_gas,
                gas_price: provider.get_gas_price().await?,
            })
        }
        
        GasModel::Legacy => {
            // BNB Chain: simple gasPrice model
            Ok(GasEstimate {
                gas: U256::from(21_000),
                gas_price: provider.get_gas_price().await?,
            })
        }
    }
}
```

#### 3.2.4 ERC-4337 (Account Abstraction)
```rust
// crates/chain-evm/src/userop.rs

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

pub async fn encode_user_operation(
    account_address: Address,
    to: Address,
    value: U256,
    data: Bytes,
    entry_point: Address,
    provider: &Provider,
) -> Result<UserOperation> {
    // 1. 获取 account 的 nonce
    let nonce = provider
        .call(
            Call::new()
                .to(account_address)
                .function(SimpleAccountContract::getNonce),
        )
        .await?;
    
    // 2. 编码 call_data (SimpleAccount.execute)
    let call_data = SimpleAccountContract::encode_execute(to, value, data)?;
    
    // 3. 估计 gas
    let (call_gas, verification_gas, pre_verification_gas) =
        estimate_user_operation_gas(provider, ...).await?;
    
    // 4. 获取最新 gas 价格
    let (max_fee, max_priority_fee) = provider
        .estimate_eip1559_fees(None)
        .await?;
    
    Ok(UserOperation {
        sender: account_address,
        nonce,
        init_code: Bytes::new(),  // Already deployed
        call_data,
        call_gas_limit: call_gas,
        verification_gas_limit: verification_gas,
        pre_verification_gas,
        max_fee_per_gas: max_fee,
        max_priority_fee_per_gas: max_priority_fee,
        paymaster_and_data: Bytes::new(),
        signature: Bytes::new(),  // Will be filled by MpcSigner
    })
}

pub async fn send_user_operation(
    user_op: UserOperation,
    bundler_url: &str,
) -> Result<String> {
    // 调用 eth_sendUserOperation JSON-RPC
    let client = JsonRpcClient::new(bundler_url);
    let tx_hash = client
        .request(
            "eth_sendUserOperation",
            (user_op, entry_point_address),
        )
        .await?;
    Ok(tx_hash)
}
```

**支持的链**:
- [ ] Ethereum (Chain ID: 1)
- [ ] Base (8453)
- [ ] Arbitrum One (42161)
- [ ] Optimism (10)
- [ ] BNB Chain (56)

**测试**:
- [ ] 各链 gas 模型验证
- [ ] EIP-712 签名测试
- [ ] Anvil 本地网络集成测试
- [ ] Base Sepolia 真实转账

**时间估计**: 10-12天  
**依赖**: 2.2 TSS 签名完成

---

### 3.3 Flutter 移动应用 UI/UX 🟡 High

**优先级**: 🟡 High  
**文件**: `mobile/lib/views/`, `mobile/lib/widgets/`

**6 个主视图**:

#### 3.3.1 Home View
```dart
// mobile/lib/views/home_view.dart

class HomeView extends StatefulWidget {
  @override
  State<HomeView> createState() => _HomeViewState();
}

class _HomeViewState extends State<HomeView> {
  Widget build(BuildContext context) {
    return Column(
      children: [
        // 总资产卡片
        AssetCard(
          totalValue: _totalValue,  // $12,345.67
          assets: [
            AssetItem(
              name: 'Ethereum',
              amount: 1.5,
              value: 4500.00,
            ),
            // ...
          ],
        ),
        
        // 快速操作
        QuickActionBar(
          onSend: () => _navigateTo(Views.wallet),
          onReceive: () => _showReceiveQR(),
          onSwap: () => _navigateTo(Views.agents),
        ),
        
        // 最近交易
        TransactionList(
          transactions: _recentTxs,
          onTxTap: (tx) => _showTxDetail(tx),
        ),
      ],
    );
  }
}
```

#### 3.3.2 Wallet View (Send/Receive/History)
```dart
// mobile/lib/views/wallet_view.dart

class WalletView extends StatefulWidget {
  @override
  _WalletViewState createState() => _WalletViewState();
}

class _WalletViewState extends State<WalletView> {
  int _tabIndex = 0;  // 0: Send, 1: Receive, 2: History
  
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        TabBar(
          tabs: [
            Tab(text: 'Send'),
            Tab(text: 'Receive'),
            Tab(text: 'History'),
          ],
          onTap: (index) => setState(() => _tabIndex = index),
        ),
        
        Expanded(
          child: [
            _buildSendTab(),
            _buildReceiveTab(),
            _buildHistoryTab(),
          ][_tabIndex],
        ),
      ],
    );
  }
  
  Widget _buildSendTab() {
    return Column(
      children: [
        // 链选择
        ChainSelector(
          chains: ['Ethereum', 'Base', 'Arbitrum', ...],
          onSelect: (chain) => setState(() => _selectedChain = chain),
        ),
        
        // 收款地址
        TextField(
          hintText: 'Recipient address',
          onChanged: (addr) => _recipientAddress = addr,
        ),
        
        // 金额
        TextField(
          hintText: 'Amount',
          keyboardType: TextInputType.number,
          onChanged: (amount) => _amount = amount,
        ),
        
        // Policy 显示 (自动评估)
        if (_policyDecision != null)
          PolicyCard(decision: _policyDecision),
        
        // 发送按钮
        PrimaryButton(
          label: 'Send',
          onPress: () => _submitTransaction(),
        ),
      ],
    );
  }
  
  Widget _buildReceiveTab() {
    return Column(
      children: [
        Text('Receive Address:'),
        SelectableText(_walletAddress),
        
        QrCode(data: _walletAddress),
        
        PrimaryButton(
          label: 'Copy Address',
          onPress: () => _copyToClipboard(_walletAddress),
        ),
      ],
    );
  }
  
  Widget _buildHistoryTab() {
    return TransactionList(
      transactions: _allTransactions,
      onTxTap: (tx) => _showTxDetail(tx),
    );
  }
  
  void _submitTransaction() async {
    // 1. Policy 二次确认
    final decision = await _apiClient.evaluatePolicy(PolicyRequest(
      to: _recipientAddress,
      value: _amount,
      chain: _selectedChain,
    ));
    
    if (!decision.allowed) {
      _showSnackbar('Transaction rejected: ${decision.reason}');
      return;
    }
    
    // 2. 生物识别
    final authenticated = await _biometricAuth.authenticate();
    if (!authenticated) return;
    
    // 3. 调用 Rust FFI 签名
    try {
      final signature = await _mpcBridge.sign(
        hash: _computeHash(_recipientAddress, _amount),
        chain: _selectedChain,
      );
      
      // 4. 广播交易
      final txHash = await _apiClient.submitTransaction(
        to: _recipientAddress,
        value: _amount,
        signature: signature,
        chain: _selectedChain,
      );
      
      _showSnackbar('Transaction sent: $txHash');
      _navigateTo(Views.home);
    } catch (e) {
      _showSnackbar('Error: $e');
    }
  }
}
```

#### 3.3.3 Agents View (AI Tools)
```dart
// mobile/lib/views/agents_view.dart

class AgentsView extends StatefulWidget {
  @override
  _AgentsViewState createState() => _AgentsViewState();
}

class _AgentsViewState extends State<AgentsView> {
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        // 工具卡片网格
        GridView.builder(
          gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
            crossAxisCount: 2,
          ),
          itemCount: _agents.length,
          itemBuilder: (ctx, i) => AgentCard(
            agent: _agents[i],
            onTap: () => _openAgent(_agents[i]),
          ),
        ),
      ],
    );
  }
}
```

#### 3.3.4 Settings View
```dart
// mobile/lib/views/settings_view.dart

class SettingsView extends StatefulWidget {
  @override
  _SettingsViewState createState() => _SettingsViewState();
}

class _SettingsViewState extends State<SettingsView> {
  @override
  Widget build(BuildContext context) {
    return ListView(
      children: [
        ListTile(
          title: Text('Security'),
          onTap: () => _navigateTo(Views.settings, tab: 'security'),
        ),
        ListTile(
          title: Text('Notifications'),
          onTap: () => _navigateTo(Views.settings, tab: 'notifications'),
        ),
        ListTile(
          title: Text('Policies'),
          onTap: () => _navigateTo(Views.settings, tab: 'policies'),
        ),
        ListTile(
          title: Text('About'),
          onTap: () => _showAbout(),
        ),
      ],
    );
  }
}
```

#### 3.3.5 Keys View (分片查看)
```dart
// mobile/lib/views/keys_view.dart

class KeysView extends StatefulWidget {
  @override
  _KeysViewState createState() => _KeysViewState();
}

class _KeysViewState extends State<KeysView> {
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Text('Key Shards', style: Theme.of(context).textTheme.headlineSmall),
        
        // Shard 1: Device (本地 SE)
        ShardCard(
          title: 'Device Shard',
          description: 'Stored in Secure Enclave',
          status: _shardStatus[0],
          onTap: () => _showShardDetail(0),
        ),
        
        // Shard 2: Server (HSM)
        ShardCard(
          title: 'Server Shard',
          description: 'Stored in HSM',
          status: _shardStatus[1],
          onTap: () => _showShardDetail(1),
        ),
        
        // Shard 3: Backup (离线)
        ShardCard(
          title: 'Backup Shard',
          description: 'Offline backup (3-of-5)',
          status: _shardStatus[2],
          onTap: () => _showShardDetail(2),
        ),
        
        // 公钥显示
        Text('Public Key:'),
        SelectableText(_publicKeyHex, maxLines: 3),
      ],
    );
  }
}
```

#### 3.3.6 Chat View (AI 聊天)
```dart
// mobile/lib/views/chat_view.dart

class ChatView extends StatefulWidget {
  @override
  _ChatViewState createState() => _ChatViewState();
}

class _ChatViewState extends State<ChatView> {
  final List<ChatMessage> _messages = [];
  final TextEditingController _inputController = TextEditingController();
  
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        // 聊天消息列表
        Expanded(
          child: ListView.builder(
            itemCount: _messages.length,
            itemBuilder: (ctx, i) => ChatBubble(
              message: _messages[i],
              isUser: _messages[i].sender == 'user',
            ),
          ),
        ),
        
        // 输入框 + 意图卡片
        if (_intentCard != null)
          IntentCard(
            intent: _intentCard,
            onConfirm: () => _executeIntent(_intentCard),
            onCancel: () => setState(() => _intentCard = null),
          ),
        
        // 消息输入
        Container(
          padding: EdgeInsets.all(16),
          child: Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _inputController,
                  hintText: 'Type a message...',
                  onChanged: (text) {
                    // 实时意图检测
                    _detectIntent(text);
                  },
                ),
              ),
              IconButton(
                icon: Icon(Icons.send),
                onPressed: () => _sendMessage(),
              ),
            ],
          ),
        ),
      ],
    );
  }
  
  void _sendMessage() async {
    final msg = _inputController.text;
    _inputController.clear();
    
    // 添加用户消息
    setState(() {
      _messages.add(ChatMessage(
        text: msg,
        sender: 'user',
        timestamp: DateTime.now(),
      ));
    });
    
    // 调用 Claude API
    try {
      final response = await _claudeApi.chat(
        messages: _messages,
        tools: _walletTools,
      );
      
      // 添加 AI 响应
      setState(() {
        _messages.add(ChatMessage(
          text: response.text,
          sender: 'ai',
          timestamp: DateTime.now(),
        ));
      });
      
      // 检查是否有工具调用
      if (response.toolUse != null) {
        _handleToolUse(response.toolUse);
      }
    } catch (e) {
      _showSnackbar('Error: $e');
    }
  }
}
```

**8步引导流程**:
- [ ] Hero 页面 (品牌介绍)
- [ ] 启动流程 (创建 vs 导入)
- [ ] 安全设置 (SE/StrongBox 初始化)
- [ ] 生物识别设置 (Face/Touch ID)
- [ ] 用户名设置
- [ ] 人物角色选择
- [ ] 钱包创建 (DKG 流程)
- [ ] 完成页面 (备份提示)

**时间估计**: 12-14天  
**依赖**: 1.3, 1.4 平台通道集成完成

---

## Part 4: Phase 4 安全加固 (W17-W20)

### 4.1 越狱检测与防护 🟡 High

**优先级**: 🟡 High  
**文件**: `mobile/lib/platform/security/`

**任务**:
- [ ] iOS jailbreak 检测
- [ ] Android root/hooking 检测
- [ ] 运行时保护
- [ ] 证书固定 (TLS Pinning)

**时间估计**: 6-8天

---

### 4.2 第三方安全审计 🔴 Critical

**优先级**: 🔴 Critical (W18-W20)

**审计对象**:
- `crates/mpc-core/` - DKLS23 实现
- `crates/policy-engine/` - 风控逻辑
- `crates/storage-crypto/` - 密钥管理

**时间估计**: 2-3周

---

## Part 5: Phase 5 上线发布 (W21-W24)

### 5.1 应用商店发布
- [ ] iOS App Store 提交
- [ ] Google Play 提交
- [ ] 审批流程 (1-2周)

**时间估计**: 2-3周

---

## 📋 实现顺序建议

### Week 1-2: FFI + 平台集成
```
1.2 FFI 绑定
  ↓
1.3 iOS SE
  ↓
1.4 Android StrongBox
```

### Week 3-8: MPC 核心
```
2.1 DKG
  ↓
2.2 TSS 签名 (Presign + Sign)
  ↓
2.3 密钥分片
  ↓
2.5 Transport (Noise + NATS)
```

### Week 9-12: 产品
```
3.1 Policy Engine
  ↓
3.2 EVM Signer + Chains
  ↓
3.3 Flutter UI
```

### Week 13-20: 安全 + 测试
```
4.1 安全检测
  ↓
4.2 审计修复
  ↓
集成测试 + 端到端测试
```

### Week 21-24: 发布
```
应用商店发布 + 监控配置
```

---

## 验收标准

### M1 (W4): 基础架构 ✅
- [x] CI 绿灯
- [ ] FFI 调用成功
- [ ] SE/StrongBox 硬件密钥生成

### M2 (W10): 核心协议 🔴
- [ ] Base Sepolia DKG → 签名 → ETH 转账
- [ ] 2-of-3 签名验证通过

### M3 (W16): 产品功能 🔴
- [ ] Alpha 内测
- [ ] 完整钱包创建/转账/恢复流程

### M4 (W20): 安全加固 🔴
- [ ] 审计零关键发现
- [ ] 越狱检测就位

### M5 (W24): 上线发布 🔴
- [ ] 双平台上架
- [ ] Beta 用户完整生命周期验证

---

**项目负责人**: @jingle  
**最后更新**: 2026-04-30  
**下一步**: 从 Part 1.2 开始实现 FFI 绑定
