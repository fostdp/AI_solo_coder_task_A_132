# ---- Build stage ----
FROM rust:alpine3.20 AS builder

RUN apk add --no-cache \
    musl-dev \
    perl \
    make \
    cmake \
    gcc \
    g++ \
    pkgconfig \
    openssl-dev \
    ca-certificates

WORKDIR /app

RUN rustup target add x86_64-unknown-linux-musl

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY config ./config

ENV OPENSSL_STATIC=1 \
    OPENSSL_DIR=/usr \
    PKG_CONFIG_ALLOW_CROSS=1 \
    RUSTFLAGS="-C target-feature=+crt-static"

RUN cargo build --release --target x86_64-unknown-linux-musl

# ---- Runtime stage ----
FROM alpine:3.20

RUN apk add --no-cache \
    ca-certificates \
    tzdata

ENV TZ=Asia/Shanghai \
    RUST_LOG=info

WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/clepsydra-backend /app/clepsydra-backend
COPY --from=builder /app/config /app/config

RUN chmod +x /app/clepsydra-backend

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -qO- http://localhost:8080/api/status || exit 1

CMD ["/app/clepsydra-backend"]
