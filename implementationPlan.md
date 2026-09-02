# E-Commerce Platform — Implementation Plan
**Stack:** Rust (Axum + Tokio) · PostgreSQL (database-per-service) · Redis · Docker & Docker Compose · Traefik · Stripe · SendGrid

---

## 1. Architecture Overview

A microservices e-commerce platform composed of six independent Rust services, each with its own datastore, communicating over REST, fronted by an API gateway.

```
                         ┌─────────────┐
                         │   Traefik   │  (API Gateway, port 80)
                         └──────┬──────┘
        ┌───────────┬──────────┼──────────┬───────────┬─────────────┐
        │           │          │          │           │             │
   ┌────▼───┐  ┌────▼────┐┌────▼────┐┌────▼────┐┌─────▼─────┐┌──────▼──────┐
   │  User  │  │ Product ││  Cart   ││  Order  ││  Payment  ││Notification │
   │Service │  │ Service ││ Service ││ Service ││  Service  ││   Service   │
   │ :8081  │  │  :8082  ││  :8083  ││  :8084  ││   :8085   ││    :8086    │
   └────┬───┘  └────┬────┘└────┬────┘└────┬────┘└─────┬─────┘└─────────────┘
        │           │          │          │           │
   ┌────▼───┐  ┌────▼────┐┌────▼────┐┌────▼────┐┌─────▼─────┐
   │Postgres│  │Postgres ││ Redis   ││Postgres ││ Postgres  │
   │ users  │  │products ││  cart   ││ orders  ││ payments  │
   └────────┘  └─────────┘└─────────┘└─────────┘└───────────┘
```

Order Service is the orchestrator: on checkout it calls User, Product, Cart, Payment, and Notification services in sequence (a manual **saga pattern** — no distributed transactions, just sequential calls with compensating rollback actions on failure).

---

## 2. Repository / Workspace Layout

```
ecommerce-platform/
├── Cargo.toml                    # workspace manifest
├── docker-compose.yml
├── .env                          # secrets, not committed
├── .gitignore
├── services/
│   ├── user-service/
│   │   ├── Cargo.toml
│   │   ├── Dockerfile
│   │   ├── migrations/
│   │   └── src/
│   │       ├── main.rs
│   │       ├── routes.rs
│   │       ├── handlers/
│   │       │   ├── register.rs
│   │       │   ├── login.rs
│   │       │   └── get_user.rs
│   │       ├── models.rs
│   │       ├── db.rs
│   │       ├── auth.rs
│   │       └── error.rs
│   ├── product-service/          (same internal shape)
│   ├── cart-service/
│   ├── order-service/
│   ├── payment-service/
│   └── notification-service/
├── libs/
│   └── common/                   # shared crate: AppError, JWT verify, Claims, config loader
└── gateway/                      # Traefik dynamic config, if not fully label-driven
```

**Root `Cargo.toml`:**
```toml
[workspace]
resolver = "2"
members = [
  "services/user-service",
  "services/product-service",
  "services/cart-service",
  "services/order-service",
  "services/payment-service",
  "services/notification-service",
  "libs/common",
]
```

`libs/common` is a library crate (not a binary). It holds code every service needs: an `AppError` enum implementing `axum::response::IntoResponse`, the JWT `Claims` struct plus encode/decode helpers, and a shared config/env loader. Each service depends on it as a path dependency:
```toml
common = { path = "../../libs/common" }
```

---

## 3. Shared Dependencies (per-service `Cargo.toml`)

```toml
[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["trace", "cors"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-rustls", "macros", "chrono", "uuid"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
dotenvy = "0.15"
thiserror = "1"
reqwest = { version = "0.12", features = ["json"] }
common = { path = "../../libs/common" }
```

**Service-specific additions:**
- User Service: `jsonwebtoken = "9"`, `argon2 = "0.5"`
- Cart Service: `redis = { version = "0.25", features = ["tokio-comp"] }`
- Payment Service: `async-stripe = "0.37"` (or raw `reqwest` calls to Stripe's REST API)
- Notification Service: `lettre = "0.11"` (SMTP) — or plain `reqwest` calls to SendGrid's API

---

## 4. Service Specifications

### 4.1 User Service (port 8081)

**Table `users`:**
```sql
CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email TEXT UNIQUE NOT NULL,
  password_hash TEXT NOT NULL,
  full_name TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

**Endpoints:**
| Method | Path | Auth | Description |
|---|---|---|---|
| POST | `/register` | none | `{ email, password, full_name }` → 201, returns `{ id, email, full_name }` |
| POST | `/login` | none | `{ email, password }` → 200, returns `{ token, expires_at }` |
| GET | `/users/:id` | JWT | returns profile; 401 if invalid/missing token |
| GET | `/health` | none | returns 200 `"ok"` |

**JWT claims:**
```rust
struct Claims {
    sub: String,   // user id
    email: String,
    exp: usize,    // unix timestamp
    iat: usize,
}
```
Signed with HS256 using `JWT_SECRET`, shared via env var with every service that needs to verify tokens (Cart, Order).

**Password hashing:** `argon2::Argon2` with default parameters — never hand-roll hashing.

---

### 4.2 Product Catalog Service (port 8082)

**Tables:**
```sql
CREATE TABLE categories (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name TEXT NOT NULL UNIQUE
);

CREATE TABLE products (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  category_id UUID REFERENCES categories(id),
  name TEXT NOT NULL,
  description TEXT,
  price_cents INTEGER NOT NULL,
  stock_quantity INTEGER NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

**Endpoints:**
| Method | Path | Auth | Description |
|---|---|---|---|
| GET | `/products?category=&page=&limit=` | none | paginated list |
| GET | `/products/:id` | none | single product |
| POST | `/products` | JWT (admin role) | create product |
| PATCH | `/products/:id/stock` | internal | `{ delta: i32 }` — atomic stock adjustment |
| GET | `/health` | none | health check |

Stock update must be atomic and fail cleanly on insufficient stock:
```sql
UPDATE products
SET stock_quantity = stock_quantity + $1
WHERE id = $2 AND stock_quantity + $1 >= 0
RETURNING stock_quantity;
```
Zero rows returned = insufficient stock; handle as a 409 Conflict.

---

### 4.3 Shopping Cart Service (port 8083) — Redis-backed

**Data model:** one Redis hash per user — key `cart:{user_id}`, field `product_id`, value `quantity`. TTL of 30 days via `EXPIRE` so abandoned carts self-clean.

**Endpoints:**
| Method | Path | Auth | Description |
|---|---|---|---|
| GET | `/cart` | JWT | `{ items: [{ product_id, quantity }] }` |
| POST | `/cart/items` | JWT | `{ product_id, quantity }` → `HINCRBY` |
| DELETE | `/cart/items/:product_id` | JWT | `HDEL` |
| DELETE | `/cart` | JWT | `DEL cart:{user_id}` (post-checkout) |
| GET | `/health` | none | health check |

---

### 4.4 Order Service (port 8084) — the orchestrator

**Tables:**
```sql
CREATE TABLE orders (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending', -- pending, paid, shipped, delivered, cancelled
  total_cents INTEGER NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE order_items (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id UUID REFERENCES orders(id),
  product_id UUID NOT NULL,
  quantity INTEGER NOT NULL,
  unit_price_cents INTEGER NOT NULL
);
```

**Checkout flow — `POST /orders`:**
1. Verify JWT, extract `user_id`.
2. Call Cart Service (`GET http://cart-service:8083/cart`), forwarding the same `Authorization` header.
3. For each cart item: call Product Service for current price, then `PATCH /products/:id/stock` to decrement. If any item fails, roll back prior decrements (increment them back) and return 409.
4. Insert `orders` + `order_items` in a single SQL transaction, status `pending`.
5. Call Payment Service to charge the user.
6. On success: set order status `paid`, call Cart Service to clear the cart, call Notification Service to send order confirmation.
7. On payment failure: set order status `cancelled`, roll back all stock decrements.

**Endpoints:**
| Method | Path | Auth | Description |
|---|---|---|---|
| POST | `/orders` | JWT | checkout flow above |
| GET | `/orders` | JWT | current user's order history |
| GET | `/orders/:id` | JWT | single order detail |
| PATCH | `/orders/:id/status` | internal/admin | e.g. mark shipped |
| GET | `/health` | none | health check |

---

### 4.5 Payment Service (port 8085)

Uses Stripe **test mode** (free sandbox, dedicated test keys).

**Table:**
```sql
CREATE TABLE payments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id UUID NOT NULL,
  stripe_payment_intent_id TEXT,
  amount_cents INTEGER NOT NULL,
  status TEXT NOT NULL, -- succeeded, failed, pending
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

**Endpoints:**
| Method | Path | Auth | Description |
|---|---|---|---|
| POST | `/payments` | internal | `{ order_id, amount_cents }` → creates Stripe PaymentIntent, inserts row, returns `{ status, payment_id }` |
| POST | `/webhooks/stripe` | Stripe signature | receives `payment_intent.succeeded`/`failed` events, verifies via `STRIPE_WEBHOOK_SECRET`, updates row |
| GET | `/health` | none | health check |

Required env vars: `STRIPE_SECRET_KEY`, `STRIPE_WEBHOOK_SECRET`.

---

### 4.6 Notification Service (port 8086)

Stateless — no database.

**Endpoints:**
| Method | Path | Auth | Description |
|---|---|---|---|
| POST | `/notify/email` | internal | `{ to, subject, body }` → sends via SendGrid REST API (or SMTP via `lettre`) |
| GET | `/health` | none | health check |

Called synchronously by Order Service. Candidate for decoupling via a message queue later.

Required env var: `SENDGRID_API_KEY`.

---

## 5. Dockerfile Template (identical shape per service)

`services/user-service/Dockerfile`:
```dockerfile
# --- build stage ---
FROM rust:1.79 AS builder
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
```
Build context must be the **repo root** so it can see `libs/common`:
```
docker build -f services/user-service/Dockerfile .
```
Multi-stage build keeps the final image small — Rust compiles into a heavy builder image, but only the compiled binary is copied into the slim runtime image.

---

## 6. docker-compose.yml (full)

```yaml
version: "3.9"

services:
  user-db:
    image: postgres:16
    environment:
      POSTGRES_USER: user_svc
      POSTGRES_PASSWORD: user_svc_pw
      POSTGRES_DB: users
    volumes:
      - user_db_data:/var/lib/postgresql/data

  user-service:
    build:
      context: .
      dockerfile: services/user-service/Dockerfile
    environment:
      DATABASE_URL: postgres://user_svc:user_svc_pw@user-db:5432/users
      JWT_SECRET: ${JWT_SECRET}
    depends_on:
      - user-db
    ports:
      - "8081:8081"

  product-db:
    image: postgres:16
    environment:
      POSTGRES_USER: product_svc
      POSTGRES_PASSWORD: product_svc_pw
      POSTGRES_DB: products
    volumes:
      - product_db_data:/var/lib/postgresql/data

  product-service:
    build:
      context: .
      dockerfile: services/product-service/Dockerfile
    environment:
      DATABASE_URL: postgres://product_svc:product_svc_pw@product-db:5432/products
    depends_on:
      - product-db
    ports:
      - "8082:8082"

  cart-redis:
    image: redis:7-alpine
    volumes:
      - cart_redis_data:/data

  cart-service:
    build:
      context: .
      dockerfile: services/cart-service/Dockerfile
    environment:
      REDIS_URL: redis://cart-redis:6379
      JWT_SECRET: ${JWT_SECRET}
    depends_on:
      - cart-redis
    ports:
      - "8083:8083"

  order-db:
    image: postgres:16
    environment:
      POSTGRES_USER: order_svc
      POSTGRES_PASSWORD: order_svc_pw
      POSTGRES_DB: orders
    volumes:
      - order_db_data:/var/lib/postgresql/data

  order-service:
    build:
      context: .
      dockerfile: services/order-service/Dockerfile
    environment:
      DATABASE_URL: postgres://order_svc:order_svc_pw@order-db:5432/orders
      JWT_SECRET: ${JWT_SECRET}
      USER_SERVICE_URL: http://user-service:8081
      PRODUCT_SERVICE_URL: http://product-service:8082
      CART_SERVICE_URL: http://cart-service:8083
      PAYMENT_SERVICE_URL: http://payment-service:8085
      NOTIFICATION_SERVICE_URL: http://notification-service:8086
    depends_on:
      - order-db
      - user-service
      - product-service
      - cart-service
    ports:
      - "8084:8084"

  payment-db:
    image: postgres:16
    environment:
      POSTGRES_USER: payment_svc
      POSTGRES_PASSWORD: payment_svc_pw
      POSTGRES_DB: payments
    volumes:
      - payment_db_data:/var/lib/postgresql/data

  payment-service:
    build:
      context: .
      dockerfile: services/payment-service/Dockerfile
    environment:
      DATABASE_URL: postgres://payment_svc:payment_svc_pw@payment-db:5432/payments
      STRIPE_SECRET_KEY: ${STRIPE_SECRET_KEY}
      STRIPE_WEBHOOK_SECRET: ${STRIPE_WEBHOOK_SECRET}
    depends_on:
      - payment-db
    ports:
      - "8085:8085"

  notification-service:
    build:
      context: .
      dockerfile: services/notification-service/Dockerfile
    environment:
      SENDGRID_API_KEY: ${SENDGRID_API_KEY}
    ports:
      - "8086:8086"

volumes:
  user_db_data:
  product_db_data:
  cart_redis_data:
  order_db_data:
  payment_db_data:
```

`.env` at repo root (git-ignored):
```
JWT_SECRET=some-long-random-string
STRIPE_SECRET_KEY=sk_test_xxx
STRIPE_WEBHOOK_SECRET=whsec_xxx
SENDGRID_API_KEY=SG.xxx
```

**Important:** `depends_on` only controls container *start order*, not DB readiness — Postgres takes a few seconds to accept connections after starting. Each service should retry its DB connection a few times with backoff in `main.rs` rather than crashing immediately on first failure.

---

## 7. API Gateway (Traefik)

Added once every service works individually. In `docker-compose.yml`:
```yaml
  gateway:
    image: traefik:v3.0
    command:
      - "--api.insecure=true"
      - "--providers.docker=true"
      - "--providers.docker.exposedbydefault=false"
      - "--entrypoints.web.address=:80"
    ports:
      - "80:80"
      - "8080:8080"   # Traefik dashboard
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
```
Each service then gets routing labels instead of a direct host port mapping:
```yaml
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.user-service.rule=PathPrefix(`/api/users`)"
      - "traefik.http.services.user-service.loadbalancer.server.port=8081"
```
All traffic then flows through `http://localhost/api/users/...`, `/api/products/...`, etc.

---

## 8. Observability

- **Logging:** `tracing` + `tracing-subscriber` in every service, structured JSON or pretty logs to stdout, viewable via `docker compose logs -f <service>`.
- **Metrics (later):** add the `metrics` + `metrics-exporter-prometheus` crates to expose a `/metrics` endpoint per service; run Prometheus + Grafana as additional Compose services to scrape and visualize.
- **Log aggregation (later, heavier):** Grafana Loki is a lighter, Docker-friendlier alternative to the full ELK stack; move to ELK only if you specifically want Elasticsearch-based log search.

---

## 9. Inter-Service Communication Notes

- Start with plain REST via `reqwest`, calling other services by their Compose service name (e.g. `http://product-service:8082`) — Docker's internal DNS resolves this automatically.
- Introduce a message broker (RabbitMQ or Redis Streams) only once you feel the pain of tight coupling — e.g. when Notification Service being down blocks checkout. At that point, Order Service publishes an "order placed" event instead of calling Notification Service directly.

---

## 10. CI/CD (GitHub Actions)

Basic pipeline per push:
1. `cargo test` for each service
2. `cargo clippy -- -D warnings` for lint enforcement
3. Build each service's Docker image
4. Push images to GitHub Container Registry (free, integrates directly with GitHub Actions auth)

---

## 11. Build Order (sequential — do not parallelize as a beginner)

1. User Service — run locally with `cargo run`, no Docker yet
2. Dockerize User Service + its Postgres DB via Compose
3. Product Service — repeat the same loop
4. Cart Service — introduces Redis
5. Order Service — introduces inter-service HTTP calls (the core "microservices" milestone)
6. Payment Service — introduces third-party API integration (Stripe)
7. Notification Service
8. Traefik API Gateway wiring
9. Tracing/logging polish
10. CI pipeline
11. Kubernetes migration (only once Compose feels comfortable — k3s or minikube locally)

---

## 12. Environment Variables Reference

| Variable | Used by | Purpose |
|---|---|---|
| `DATABASE_URL` | User, Product, Order, Payment | Postgres connection string |
| `REDIS_URL` | Cart | Redis connection string |
| `JWT_SECRET` | User, Cart, Order | Shared HS256 signing key |
| `STRIPE_SECRET_KEY` | Payment | Stripe API auth |
| `STRIPE_WEBHOOK_SECRET` | Payment | Verifies incoming Stripe webhooks |
| `SENDGRID_API_KEY` | Notification | Email sending |
| `USER_SERVICE_URL`, `PRODUCT_SERVICE_URL`, `CART_SERVICE_URL`, `PAYMENT_SERVICE_URL`, `NOTIFICATION_SERVICE_URL` | Order | Internal service base URLs |
