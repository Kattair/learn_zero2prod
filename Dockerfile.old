# Builder stage
FROM rust:bookworm AS builder

WORKDIR /app

RUN apt update && apt install -y mold musl-tools musl-dev
RUN rustup component add rust-std-x86_64-unknown-linux-musl

COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --target x86_64-unknown-linux-musl

# Runtime stage
FROM alpine AS runtime

WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/zero2prod zero2prod
COPY configuration configuration

ENV APP_ENVIRONMENT=production
ENTRYPOINT [ "./zero2prod" ]