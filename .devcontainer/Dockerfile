ARG RUST_VERSION
FROM mcr.microsoft.com/devcontainers/rust:${RUST_VERSION}

RUN curl https://bootstrap.pypa.io/get-pip.py -o /tmp/get-pip.py
RUN python3 /tmp/get-pip.py
RUN pip install jupyter

RUN mkdir /usr/local/share/jupyter/kernels/learning-rust
RUN echo '{ \
    "argv": [ \
        "/tmp/cargo/debug/jupyter-kernel", \
        "--path", "{connection_file}", \
        "--cluster-id", "1024-181223-ye0dza2w" \
    ], \
    "display_name": "Learning Rust", \
    "language": "python", \
    "metadata": { \
        "debugger": false \
    } \
}' > /usr/local/share/jupyter/kernels/learning-rust/kernel.json