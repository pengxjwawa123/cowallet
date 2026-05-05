# 多阶段构建：依赖层 + 代码层
FROM rust:1.75-slim AS builder

WORKDIR /app

# 安装依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# 1. 先创建空的 lib.rs 构建依赖层（这层可缓存）
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p crates/mpc-core/src crates/chain-evm/src crates/policy-engine/src crates/ai-bridge/src \
    && echo "fn main() {}" > crates/mpc-core/src/lib.rs \
    && echo "fn main() {}" > crates/chain-evm/src/lib.rs \
    && echo "fn main() {}" > crates/policy-engine/src/lib.rs \
    && echo "fn main() {}" > crates/ai-bridge/src/lib.rs \
    && mkdir -p backend/api-server/src backend/mpc-relay/src backend/indexer/src backend/worker/src \
    && echo "fn main() {}" > backend/api-server/src/main.rs \
    && echo "fn main() {}" > backend/mpc-relay/src/main.rs \
    && echo "fn main() {}" > backend/indexer/src/main.rs \
    && echo "fn main() {}" > backend/worker/src/main.rs

# 只构建依赖（这层可缓存数周）
RUN cargo build --release --bin api-server --bin mpc-relay --bin indexer --bin worker 2>/dev/null || true

# 2. 复制真实源代码并构建
COPY crates ./crates
COPY backend ./backend
COPY migrations ./migrations

# 构建最终二进制文件
RUN cargo build --release --bin api-server --bin mpc-relay --bin indexer --bin worker

# 运行层
FROM debian:bookworm-slim

WORKDIR /app

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    postgresql-client \
    && rm -rf /var/lib/apt/lists/*

# 从构建层复制二进制文件
COPY --from=builder /app/target/release/api-server /usr/local/bin/
COPY --from=builder /app/target/release/mpc-relay /usr/local/bin/
COPY --from=builder /app/target/release/indexer /usr/local/bin/
COPY --from=builder /app/target/release/worker /usr/local/bin/
COPY --from=builder /app/migrations ./migrations

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

EXPOSE 3000 4222 5432

CMD ["api-server"]
