# Cowallet API接入实施方案

## 一、项目概述
基于Rust Axum后端API，完成Flutter客户端全模块API接入，实现钱包完整功能。
- **后端地址**: `http://43.163.101.37:3000`
- **API版本**: v1
- **认证方式**: Bearer Token (JWT)

## 二、实施顺序
1. ✅ 基础网络层封装（已完成）
2. 认证模块接入
3. 钱包核心模块接入
4. 交易模块接入
5. AI功能模块接入
6. 价格行情模块接入
7. 高级功能接入（策略、MPC）

---

## 三、分模块接入方案

### 📌 模块1：认证系统 (Auth)
**接口列表**
| 接口路径 | 方法 | 业务场景 | 请求参数 | 响应处理 |
|---------|------|----------|----------|----------|
| `/auth/register` | POST | 新用户注册/设备注册 | `device_id`, `device_info`, `password(可选)` | 存储token、用户ID、钱包信息 |
| `/auth/login` | POST | 用户登录 | `device_id`, `password` | 更新token、同步用户信息 |
| `/auth/send-code` | POST | 发送验证码 | `mobile` | 提示验证码发送结果 |
| `/auth/verify-code` | POST | 验证码验证 | `mobile`, `code` | 验证通过后返回token |
| `/auth/logout` | POST | 退出登录 | 无 | 清除本地token、用户数据 |

**实施要点**
- 设备ID用设备唯一标识，客户端本地生成并持久化
- 登录/注册成功后自动存储token到SecureStorage
- 401时自动清除数据并跳转到登录页
- 支持生物识别快速登录（本地验证后自动调用登录接口）

---

### 📌 模块2：钱包核心 (Wallet)
**接口列表**
| 接口路径 | 方法 | 业务场景 | 请求参数 | 响应处理 |
|---------|------|----------|----------|----------|
| `/wallet/create` | POST | 创建新钱包 | `password(可选)` | 存储助记词（本地加密）、地址、私钥信息 |
| `/wallet/import/mnemonic` | POST | 助记词导入 | `mnemonic`, `password(可选)` | 恢复钱包地址、资产信息 |
| `/wallet/import/private-key` | POST | 私钥导入 | `private_key`, `password(可选)` | 恢复钱包 |
| `/wallet/balance` | GET | 查询余额 | `address`, `chain(可选)` | 更新首页资产列表、总市值 |
| `/wallet/tokens` | GET | 查询代币列表 | `address`, `chain(可选)` | 展示代币资产详情 |
| `/wallet/nfts` | GET | 查询NFT列表 | `address`, `chain(可选)`, `page`, `limit` | NFT画廊展示 |
| `/wallet/transactions` | GET | 交易记录 | `address`, `chain(可选)`, `page`, `limit` | 交易历史页面展示 |
| `/wallet/export/private-key` | POST | 导出私钥 | `address`, `password` | 验证密码后返回私钥 |
| `/wallet/export/mnemonic` | POST | 导出助记词 | `address`, `password` | 验证密码后返回助记词 |

**实施要点**
- 助记词/私钥仅在必要时返回，客户端加密存储，永不明文展示
- 余额、交易记录支持下拉刷新，间隔30秒自动刷新
- 多链支持：默认Base链，可切换其他EVM链
- 交易记录支持按时间、金额筛选

---

### 📌 模块3：交易系统 (Transaction)
**接口列表**
| 接口路径 | 方法 | 业务场景 | 请求参数 | 响应处理 |
|---------|------|----------|----------|----------|
| `/tx/estimate-gas` | POST | 估算Gas费用 | `from`, `to`, `amount`, `chain`, `data(可选)` | 显示Gas预估、转账手续费 |
| `/tx/gas-price` | GET | 获取当前Gas价格 | `chain` | 展示慢/标准/快三档Gas选项 |
| `/tx/transfer` | POST | 发起转账交易 | `from`, `to`, `amount`, `chain`, `token_address(可选)`, `gas_price(可选)`, `gas_limit(可选)` | 返回交易hash，跳转到交易等待页 |
| `/tx/status` | GET | 查询交易状态 | `hash`, `chain` | 轮询直到交易确认，更新交易状态 |
| `/tx/broadcast` | POST | 广播已签名交易 | `signed_tx`, `chain` | 返回交易hash |
| `/tx/speed-up` | POST | 加速交易 | `hash`, `chain`, `new_gas_price` | 返回新交易hash |
| `/tx/cancel` | POST | 取消交易 | `hash`, `chain` | 返回取消交易hash |

**实施要点**
- 交易签名在客户端本地完成，私钥不上传服务器
- 交易提交后进入轮询状态，直到确认或失败
- 支持查看交易详情、在区块浏览器打开
- 失败交易提供重试功能，重新计算Gas后再提交

---

### 📌 模块4：AI助手 (AI)
**接口列表**
| 接口路径 | 方法 | 业务场景 | 请求参数 | 响应处理 |
|---------|------|----------|----------|----------|
| `/ai/chat` | POST | AI聊天 | `message`, `conversation_id(可选)` | 流式展示AI回复内容 |
| `/ai/analysis/asset` | GET | 资产分析报告 | `address` | 生成资产构成、收益分析、风险评估报告 |
| `/ai/advice/trading` | POST | 交易建议 | `type`, `tx_params` | 给出交易时机、Gas设置、风险提示等建议 |
| `/ai/analysis/security` | GET | 安全分析 | `address` | 检查钱包授权风险、钓鱼交易记录、安全建议 |

**实施要点**
- 支持流式响应，打字机效果展示AI回复
- 分析报告支持生成图片分享
- 聊天记录本地存储，支持历史会话查看
- AI建议仅作参考，不构成投资建议，添加免责声明

---

### 📌 模块5：价格行情 (Price)
**接口列表**
| 接口路径 | 方法 | 业务场景 | 请求参数 | 响应处理 |
|---------|------|----------|----------|----------|
| `/price/current` | GET | 获取当前价格 | `token_symbols` | 更新代币价格、24h涨跌幅 |
| `/price/history` | GET | 历史价格走势 | `token`, `time_range` | 生成价格K线图 |
| `/price/market` | GET | 市场行情 | `page`, `limit` | 行情页面展示热门代币涨跌幅 |

**实施要点**
- 价格数据缓存5分钟，减少API调用
- 支持按市值、涨跌幅排序
- 价格变动超过±5%时高亮显示
- 支持添加自选代币关注

---

### 📌 模块6：策略引擎 (Policy)
**接口列表**
| 接口路径 | 方法 | 业务场景 | 请求参数 | 响应处理 |
|---------|------|----------|----------|----------|
| `/policy/list` | GET | 策略列表 | 无 | 展示用户创建的所有交易策略 |
| `/policy/create` | POST | 创建策略 | `name`, `description`, `rules`, `conditions` | 保存策略，开启/关闭开关 |
| `/policy/update` | PUT | 更新策略 | `id`, `策略字段` | 更新策略配置 |
| `/policy/delete` | DELETE | 删除策略 | `id` | 移除策略 |
| `/policy/evaluate` | POST | 评估交易 | `tx_data` | 返回策略评估结果、是否允许交易 |

**实施要点**
- 策略模板：大额转账提醒、白名单限制、每日限额、多签验证等
- 交易时自动触发策略评估，不满足条件的交易阻止执行
- 支持策略触发后通知用户（推送、APP内提醒）

---

### 📌 模块7：MPC安全计算 (MPC)
**接口列表**
| 接口路径 | 方法 | 业务场景 | 请求参数 | 响应处理 |
|---------|------|----------|----------|----------|
| `/mpc/session/create` | POST | 创建MPC会话 | `participants`, `threshold` | 返回会话ID、连接信息 |
| `/mpc/session/join` | POST | 加入MPC会话 | `session_id`, `participant_info` | 加入签名会话 |
| `/mpc/message/send` | POST | 发送MPC消息 | `session_id`, `message`, `recipient` | 消息传递 |
| `/mpc/message/receive` | GET | 接收MPC消息 | `session_id`, `last_sequence` | 轮询获取消息 |
| `/mpc/signature/complete` | POST | 完成签名 | `session_id`, `signature_data` | 返回最终签名结果 |

**实施要点**
- 用于多签交易、大额交易MPC签名
- 支持2-3，3-5等多种门限方案
- 密钥分片本地存储，永不合并
- 签名过程在内存中完成，敏感数据不留痕

---

## 四、统一处理规范

### 1. 请求处理
- 所有请求自动添加`Authorization: Bearer {token}`头
- 公共接口（登录、注册、健康检查）不需要token
- 请求参数自动做类型校验、必填项检查
- 支持请求取消、超时处理（15秒超时）

### 2. 响应处理
- 统一响应格式：`{ "code": 0, "msg": "success", "data": {} }`
- `code == 0` 表示成功，直接返回data
- `code != 0` 表示失败，抛出业务异常，显示msg内容
- HTTP状态码处理：
  - 200/201：成功
  - 400：参数错误，显示提示
  - 401：未授权，清除本地数据跳登录
  - 403：无权限，提示并返回
  - 404：接口不存在，记录日志
  - 500：服务器错误，提示稍后重试

### 3. 错误处理
- 网络错误：检查网络连接，显示"网络连接失败，请检查网络设置"
- 超时错误：显示"请求超时，请稍后重试"，支持重试按钮
- 业务错误：直接显示服务器返回的msg内容
- 未知错误：显示"系统异常，请稍后重试"，记录错误日志

### 4. 状态管理
- API返回的数据优先更新本地状态（使用ChangeNotifier）
- 核心数据（余额、交易记录）本地缓存，无网络时展示缓存数据
- 下拉刷新时强制从服务器获取最新数据
- 支持离线模式：查看缓存数据，交易提交后本地暂存，网络恢复后自动广播

---

## 五、验证标准
每个模块接入完成后需要验证以下内容：
1. ✅ 接口调用正常，无报错
2. ✅ 请求参数正确，符合后端要求
3. ✅ 响应处理正确，页面展示正常
4. ✅ 错误处理完善，各种异常情况都有友好提示
5. ✅ 状态同步正确，数据变更实时反映到UI
6. ✅ 安全合规，敏感数据加密存储，不上传明文

---

## 六、后续优化
1. 添加请求缓存，减少重复API调用
2. 实现请求重试机制，网络恢复后自动重试失败的请求
3. 添加API调用埋点，统计接口成功率、响应时间
4. 实现接口降级，部分接口失败时不影响主流程
5. 添加API限流，防止短时间内频繁调用
