FROM mcr.microsoft.com/devcontainers/rust:bookworm
USER vscode

RUN sudo apt-get update
RUN sudo apt-get upgrade -y

RUN rustup update 1.79.0
RUN rustup default 1.79.0
RUN rustup component add rustfmt
