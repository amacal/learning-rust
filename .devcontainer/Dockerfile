FROM mcr.microsoft.com/devcontainers/rust:bookworm
USER vscode

RUN sudo apt-get update
RUN sudo apt-get upgrade -y

RUN rustup update 1.77.1
RUN rustup default 1.77.1
RUN rustup component add rustfmt
RUN rustup target add x86_64-unknown-none
RUN cargo install cargo-expand
