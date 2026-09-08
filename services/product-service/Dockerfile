# --- build stage ---
FROM rust:latest AS builder
WORKDIR /app

# Cache dependencies: copy manifests + shared lib only, build a dummy binary
COPY libs/common/Cargo.toml ./libs/common/Cargo.toml
COPY libs/common/src ./libs/common/src
COPY services/product-service/Cargo.toml ./services/product-service/Cargo.toml
RUN mkdir -p services/product-service/src && \
    echo 'fn main() {}' > services/product-service/src/main.rs && \
    cd services/product-service && \
    cargo build --release

# Build the real binary (deps are cached from the layer above)
COPY services/product-service/src ./services/product-service/src
COPY services/product-service/migrations ./services/product-service/migrations
RUN touch services/product-service/src/main.rs && \
    cd services/product-service && \
    cargo build --release

# --- runtime stage ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/services/product-service/target/release/product-service /usr/local/bin/product-service
EXPOSE 8082
CMD ["product-service"]