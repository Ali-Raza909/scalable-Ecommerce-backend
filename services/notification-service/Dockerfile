# --- planner stage: figure out the dependency tree ---
FROM rust:latest AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- builder stage: build deps first (cached), then real code ---
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --package notification-service --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin notification-service

# --- runtime stage ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/notification-service /usr/local/bin/notification-service
EXPOSE 8086
CMD ["notification-service"]