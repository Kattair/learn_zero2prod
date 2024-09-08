FROM mcr.microsoft.com/devcontainers/rust:1-1-bookworm

RUN apt update \
    && apt install -y postgresql mold\
    && apt clean

USER vscode
RUN cargo install sqlx-cli --no-default-features --features rustls,postgres
RUN cargo install bunyan
RUN cargo install cargo-watch
