FROM lukemathwalker/cargo-chef:latest AS chef
WORKDIR /app
RUN apt update \
    && apt install -y mold musl-tools musl-dev \
    # Clean up
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*
RUN rustup component add rust-std-x86_64-unknown-linux-musl

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --target x86_64-unknown-linux-musl --bin zero2prod

# Runtime stage
FROM alpine:latest AS runtime

WORKDIR /app

COPY configuration configuration
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/zero2prod zero2prod

ENV APP_ENVIRONMENT=production
ENTRYPOINT [ "./zero2prod" ]