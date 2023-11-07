FROM mcr.microsoft.com/devcontainers/rust:bookworm
RUN apt update && apt install -y linux-perf xxd nasm

USER vscode
RUN cargo install flamegraph