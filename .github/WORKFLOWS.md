# GitHub Actions CI/CD 配置指南

## 概述

cowallet 项目包含完整的 GitHub Actions CI/CD 流程：

1. **docker-build.yml** — 构建并推送 Docker 镜像到腾讯 TCR
2. **rust-ci.yml** — Rust 后端代码测试和检查
3. **flutter-ci.yml** — Flutter 移动应用分析和构建
4. **deploy-tke.yml** — 部署到腾讯 TKE 集群

## 必要的 GitHub Secrets 配置

访问 `Settings → Secrets and variables → Actions`，添加以下 Secrets：

### 腾讯云容器镜像服务 (TCR)

| Secret 名称 | 描述 | 获取方式 |
|------------|------|--------|
| `TCR_USERNAME` | 腾讯 TCR 用户名 | [TCR 控制台](https://console.cloud.tencent.com/tcr) |
| `TCR_PASSWORD` | 腾讯 TCR 密码 | TCR 控制台 → 访问凭证 |

### 腾讯 TKE 集群

| Secret 名称 | 描述 | 获取方式 |
|------------|------|--------|
| `TENCENT_CLOUD_ACCOUNT_ID` | 腾讯云账户 ID | [账户中心](https://console.cloud.tencent.com/developer) → 账户信息 |
| `TENCENT_CLOUD_SECRET_ID` | 腾讯云 API 密钥 ID | [CAM 控制台](https://console.cloud.tencent.com/cam/capi) → 访问密钥 |
| `TENCENT_CLOUD_SECRET_KEY` | 腾讯云 API 密钥 Secret | CAM 控制台 → 访问密钥 → API 密钥 |
| `KUBECONFIG` | Kubernetes 配置文件（Base64 编码）| 见下方设置步骤 |

**⚠️  KUBECONFIG 设置步骤：**

```bash
# 1. 获取 kubeconfig 文件
# 访问: https://console.cloud.tencent.com/tke2/cluster
# 点击集群 cls-c63h33ne → 连接信息 → 复制并保存为 kubeconfig 文件

# 2. 本地 Base64 编码
base64 kubeconfig > kubeconfig.b64
cat kubeconfig.b64  # 复制输出内容

# 3. 添加到 GitHub
# 访问: https://github.com/pengxjwawa123/cowallet/settings/secrets/actions
# 点击 "New repository secret"
# Name: KUBECONFIG
# Value: 粘贴 kubeconfig.b64 的全部内容
```

## 工作流详细说明

### 1. docker-build.yml (Docker 构建)

**触发条件：**
- 推送到 `main` 或 `develop` 分支
- 创建版本标签 (v*)
- Pull Request 到 `main`

**流程：**
```
Checkout → Setup Buildx → Login TCR → Build & Push → Security Scan
```

**镜像标签规则：**
- `main` 分支: `sgccr.ccs.tencentyun.com/cowallet:abc1234` 和 `cowallet:main`
- 版本标签: `sgccr.ccs.tencentyun.com/cowallet:v1.0.0` 和 `cowallet:latest`
- PR: `sgccr.ccs.tencentyun.com/cowallet:feature-branch-abc1234`

**输出：**
- 构建缓存保存到 GitHub Actions 缓存
- Trivy 安全扫描结果上传到 GitHub Security 标签页

### 2. rust-ci.yml (Rust 测试)

**触发条件：**
- 推送到 `main` 或 `develop`
- Pull Request 到 `main`

**流程：**
```
Checkout → Format Check → Clippy → Build → Test → Coverage
```

**测试环境：**
- PostgreSQL 16 (自动启动)
- Redis 7 (自动启动)
- NATS 2 (自动启动)

**输出：**
- 代码格式验证
- Clippy 警告检查
- Release 构建
- 全部测试运行
- 代码覆盖率上传到 Codecov

### 3. flutter-ci.yml (Flutter 构建)

**触发条件：**
- 推送到 `main` 或 `develop`（修改 `mobile/**` 目录时）
- Pull Request 到 `main`（修改 `mobile/**` 目录时）

**流程：**

Analyze (分析):
```
Checkout → Flutter Setup → Get Dependencies → Analyze → Format Check
```

Test (单元测试):
```
Checkout → Flutter Setup → Get Dependencies → Test
```

Build Android (构建 APK):
```
Checkout → Java Setup → Flutter Setup → Build APK (split-per-abi)
```

Build iOS (构建 iOS):
```
Checkout → Flutter Setup → Build iOS (no-codesign)
```

**输出：**
- 分析报告
- 测试覆盖率
- Android APK (artifacts)
- iOS 构建产物 (artifacts)

### 4. deploy-tke.yml (TKE 部署)

**触发条件：**
- 推送到 `main` 分支
- 创建版本标签 (v*)

**流程：**
```
Checkout → Setup Credentials → Get Kubeconfig → Update Deployment → Rollout Status → Slack Notification
```

**部署步骤：**
1. 获取腾讯云凭证
2. 连接 TKE 集群
3. 更新 Deployment 中的镜像
4. 等待 Rollout 完成 (5分钟超时)
5. 验证部署状态
6. 发送 Slack 通知

## 快速设置步骤

### 第一步：收集凭证

```bash
# 1. TCR 凭证
# 访问: https://console.cloud.tencent.com/tcr
# 获取: 用户名和密码

# 2. 腾讯云 API 密钥
# 访问: https://console.cloud.tencent.com/cam/capi
# 创建: 新建密钥

# 3. Kubeconfig（关键步骤⚠️）
# 访问: https://console.cloud.tencent.com/tke2/cluster?rid=1
# 点击集群 cls-c63h33ne
# 连接信息 → 复制 kubeconfig 文件内容 → 保存为 kubeconfig

# 4. Base64 编码 kubeconfig
base64 kubeconfig > kubeconfig.b64
# 重要！完整复制 kubeconfig.b64 的所有内容（包括换行符）
cat kubeconfig.b64
```

### 第二步：添加 GitHub Secrets

```bash
# 在 GitHub 项目设置中添加：
# Settings → Secrets and variables → Actions → New repository secret

# 添加以下 Secrets:
TCR_USERNAME=xxx
TCR_PASSWORD=xxx
TENCENT_CLOUD_ACCOUNT_ID=xxx
TENCENT_CLOUD_SECRET_ID=xxx
TENCENT_CLOUD_SECRET_KEY=xxx
KUBECONFIG=xxx (base64编码的内容)
```

### 第三步：验证工作流

```bash
# 推送到 main 分支触发所有工作流
git push origin main

# 查看 GitHub Actions 日志
# 访问: https://github.com/your-org/cowallet/actions
```

## 常见问题 (FAQ)

### Q: Docker 构建为什么失败？
**A:** 检查以下几点：
- Dockerfile 在项目根目录
- `docker-compose.yml` 和相关文件存在
- Docker 构建上下文配置正确

### Q: 如何跳过某个工作流？
**A:** 在提交信息中添加关键词：
```bash
git commit -m "skip ci: update docs"  # 跳过所有 CI
git commit -m "skip docker: minor fix"  # 只跳过 Docker 构建
```

### Q: 测试失败了怎么办？
**A:** 
1. 查看工作流日志获取详细错误
2. 本地运行测试：`cargo test`
3. 检查依赖版本是否正确

### Q: TKE 部署后如何回滚？
**A:**
```bash
# 使用 kubectl 回滚
kubectl rollout undo deployment/cowallet -n cowallet
```

### Q: 如何手动触发工作流？
**A:** 使用 GitHub CLI：
```bash
# 触发 Docker 构建
gh workflow run docker-build.yml

# 查看工作流列表
gh workflow list

# 查看最新运行
gh run list --workflow docker-build.yml
```

## 成本优化建议

### 1. 使用缓存减少构建时间

所有工作流已配置 GitHub Actions 缓存：
- Docker 镜像层缓存 (BuildKit cache)
- Cargo 依赖缓存
- Flutter pub 依赖缓存

### 2. 使用矩阵策略并行构建

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest]
    rust-version: [stable, nightly]
```

### 3. 仅在必要时运行昂贵操作

```yaml
if: contains(github.event.head_commit.message, '[full-test]')
```

### 4. 清理构建产物

工作流已配置自动删除 7 天前的工件。

## 监控和告警

### GitHub Actions Dashboard

访问 `Settings → Actions → General`，配置：
- 工作流超时 (默认 360 分钟)
- 日志保留策略 (默认 90 天)

## 安全最佳实践

✅ **已配置：**
- Secrets 使用 GitHub 加密存储
- 工作流权限最小化 (`permissions: read`)
- 敏感信息不会输出到日志
- 定期更新 Action 版本

⚠️  **生产建议：**
- 定期轮换 API 密钥
- 使用 OIDC 连接腾讯云 (更安全)
- 启用 Branch protection rules
- 要求代码审查后才能合并

## 下一步

1. ✅ 设置 GitHub Secrets
2. ✅ 推送到 main 分支测试工作流
3. ✅ 验证 Docker 镜像推送到 TCR
4. ✅ 测试 TKE 部署流程
5. ✅ 配置 Slack 通知 (可选)
6. ✅ 设置分支保护规则

---

**需要帮助？** 查看 GitHub Actions 官方文档：https://docs.github.com/en/actions
