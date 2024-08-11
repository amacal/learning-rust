FROM mcr.microsoft.com/devcontainers/rust:bookworm
USER vscode

RUN sudo apt-get update
RUN sudo apt-get upgrade -y
RUN sudo apt-get install -y linux-perf

RUN rustup update 1.80.1
RUN rustup update nightly
RUN rustup default 1.80.1
RUN rustup component add rustfmt
RUN rustup component add clippy
RUN rustup target add x86_64-unknown-none
RUN cargo install cargo-expand flamegraph cargo-tarpaulin
