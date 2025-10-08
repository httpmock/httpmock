# First stage: build the application
FROM rust:1.90 as builder

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Log level (refer to env_logger crate for more information)
ENV RUST_LOG httpmock=info

# The TCP port on which the mock server will listen to.
ENV HTTPMOCK_PORT 5050

# Container internal directory path that contains file bases mock specs (YAML-fies).
# ENV HTTPMOCK_MOCK_FILES_DIR /mocks

# The existence of this environment variable (even if value is empty) is considered "true"/"disabled".
# ENV HTTPMOCK_DISABLE_ACCESS_LOG true

# Request history limit.
ENV HTTPMOCK_REQUEST_HISTORY_LIMIT 100

WORKDIR /httpmock

COPY Cargo.toml .
# COPY Cargo.lock .

COPY src/ ./src/
COPY certs/ ./certs/

RUN cargo install --all-features --path .

ENTRYPOINT ["httpmock", "--expose"]

EXPOSE ${HTTPMOCK_PORT}