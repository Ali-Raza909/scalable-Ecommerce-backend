# --- build stage ---
FROM rust:latest AS builder
WORKDIR /app
COPY libs/common ./libs/common
COPY services/user-service ./services/user-service
WORKDIR /app/services/user-service
RUN cargo build --release

# --- runtime stage ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/services/user-service/target/release/user-service /usr/local/bin/user-service
EXPOSE 8081
CMD ["user-service"]
