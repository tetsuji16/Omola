FROM openmodelica/openmodelica:v1.27.1-minimal

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl build-essential git \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable

ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup component add clippy rustfmt
WORKDIR /workspaces/Omola

RUN rustc --version && cargo --version && omc --version
