# --- build stage ---
FROM rust:latest AS builder
WORKDIR /app
COPY libs/common ./libs/common
COPY services/product-service ./services/product-service
WORKDIR /app/services/product-service
RUN cargo build --release

# --- runtime stage ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/services/product-service/target/release/product-service /usr/local/bin/product-service
EXPOSE 8082
CMD ["product-service"]
