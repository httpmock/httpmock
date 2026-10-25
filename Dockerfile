# First stage: build the application
FROM rust:1.89 AS builder

WORKDIR /httpmock

COPY Cargo.toml .
# COPY Cargo.lock .

COPY src/ ./src/
COPY certs/ ./certs/

RUN cargo install --all-features --path .

FROM debian:trixie-slim

WORKDIR /httpmock

# hadolint ignore=DL3008
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/cargo/bin/httpmock /usr/local/bin/httpmock
# In 0.9, the binary moved from /usr/local/cargo/bin/httpmock (used in 0.8.3)
# to /usr/local/bin/httpmock. Keep the deprecated path working for existing
# configurations using the old absolute path. Remove in a future release.
RUN mkdir -p /usr/local/cargo/bin && ln -s /usr/local/bin/httpmock /usr/local/cargo/bin/httpmock

# Log level (refer to env_logger crate for more information)
ENV RUST_LOG=httpmock=info

# The TCP port on which the mock server will listen to.
ENV HTTPMOCK_PORT=5050

# Container internal directory path that contains file bases mock specs (YAML-fies).
# ENV HTTPMOCK_MOCK_FILES_DIR=/mocks

# The existence of this environment variable (even if value is empty) is considered "true"/"disabled".
# ENV HTTPMOCK_DISABLE_ACCESS_LOG=true

# Request history limit.
ENV HTTPMOCK_REQUEST_HISTORY_LIMIT=100

ENTRYPOINT ["httpmock", "--expose"]

EXPOSE ${HTTPMOCK_PORT}