FROM mcr.microsoft.com/devcontainers/rust:bookworm
USER vscode

RUN sudo apt-get update
RUN sudo apt-get upgrade -y
RUN sudo apt-get install -y linux-perf

RUN rustup update 1.79.0
RUN rustup update nightly
RUN rustup default nightly
RUN rustup component add rustfmt
RUN rustup target add x86_64-unknown-none
RUN cargo install cargo-expand flamegraph
