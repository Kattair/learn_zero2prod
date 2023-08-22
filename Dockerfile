FROM rust:1.71.1-bookworm

WORKDIR /app

RUN apt update && apt install -y mold

COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release

ENTRYPOINT [ "target/release/zero2prod" ]