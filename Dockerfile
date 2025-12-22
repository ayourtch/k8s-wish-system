FROM rust:1.83 as builder

WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build specific binary based on ARG
ARG BINARY_NAME
RUN cargo build --release --bin ${BINARY_NAME}

FROM debian:bookworm-slim

# Install kubectl for wish-fulfiller
RUN apt-get update && \
    apt-get install -y curl && \
    curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl" && \
    install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

ARG BINARY_NAME
COPY --from=builder /usr/src/app/target/release/${BINARY_NAME} /usr/local/bin/controller

ENTRYPOINT ["/usr/local/bin/controller"]
