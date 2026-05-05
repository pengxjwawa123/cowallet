# 多阶段构建：编译层 + 运行层
FROM rust:1.75-slim AS builder

WORKDIR /app

# 安装依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# 复制工作区配置
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./

# 复制源代码
COPY crates ./crates
COPY backend ./backend

# 构建所有二进制文件（release 优化）
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
COPY --from=builder /app/backend/api-server/migrations ./migrations

# 健康检查（仅 api-server 容器需要）
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# 导出端口
EXPOSE 3000 4222 5432

# 默认命令（可被 docker-compose 覆盖）
CMD ["api-server"]
