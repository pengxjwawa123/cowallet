# 多阶段构建：先构建依赖缓存层
FROM rust:1.75-slim AS planner
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev libpq-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked

# 复制工作区配置
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY backend ./backend
RUN cargo chef prepare --recipe-path recipe.json

# 依赖构建层（这层只有在 Cargo.toml/Cargo.lock 变化时才会重新构建）
FROM rust:1.75-slim AS cacher
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev libpq-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# 最终构建层
FROM rust:1.75-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev libpq-dev && rm -rf /var/lib/apt/lists/*

# 复制工作区配置
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./

# 复制源代码
COPY crates ./crates
COPY backend ./backend
COPY migrations ./migrations

# 从缓存层复制已构建的依赖
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

# 构建二进制文件
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
