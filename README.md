# Scalable E-Commerce Backend

A 6-service Rust microservices order pipeline behind a Traefik gateway: JWT-auth user
service, catalog + stock, Redis-backed cart, order saga, Safepay payments (sandbox),
and a Resend-backed notification service. Stock is reserved at checkout and released
via an idempotent compensation path when an order is cancelled or expires.

## Services

| Service             | Port | Storage  | Responsibility                                              |
| ------------------- | ---- | -------- | ----------------------------------------------------------- |
| user-service        | 8081 | Postgres | register/login, JWT issue & verify                         |
| product-service     | 8082 | Postgres | catalog CRUD, stock deltas                                 |
| cart-service        | 8083 | Redis    | per-user cart (JWT-protected)                              |
| order-service       | 8084 | Postgres | checkout saga, payment-confirm, admin status, timeout job  |
| payment-service     | 8085 | Postgres | Safepay session/passport, signed webhook handling          |
| notification-service| 8086 | none     | Resend email confirmations (log-only when `EMAIL_MOCK=true`) |

Each database-backed service runs `sqlx::migrate!` on boot, so a fresh Postgres gets
its schema automatically.

## Architecture

```
client ──> Traefik (:80)
             ├── /api/users/*      → user-service     (strip /api/users)
             ├── /api/products/*   → product-service  (strip /api)
             ├── /api/cart/*       → cart-service     (strip /api)
             └── /api/orders/*     → order-service    (strip /api)

order-service ──> product-service   reserve stock at checkout
              ──> cart-service      read + clear cart
              ──> payment-service   create Safepay session
              ──> notification-service (best-effort)

Safepay (sandbox) ──webhook──> payment-service ──payment-confirm──> order-service
                                    │                 │  guarded UPDATE … WHERE status='pending'
                                    │                 ▼
                                    │          paid:     notify customer
                                    │          cancelled: restore reserved stock
                                    ▼
                              payment flags: succeeded | failed | paid_after_cancel
```

Checkout order state machine: `pending → paid | cancelled`. Admin transitions:
`pending → paid | cancelled`, `paid → shipped`, `shipped → delivered`.

### The checkout saga

1. Client adds items to cart, then `POST /api/orders`.
2. order-service reserves stock (atomic decrement), reads the cart, creates a
   `pending` order, and asks payment-service to open a Safepay session + mint a
   passport token. Any failure or persist error runs the compensation path and
   releases the reserved stock.
3. The client is redirected to Safepay's hosted checkout
   (`checkout_url` from the order response).
4. payment-service verifies the signed webhook over the internal network, then
   calls order-service's internal `payment-confirm`.
5. Successful payment: order flips `pending → paid`, notification fires.
   Failed payment: order flips `pending → cancelled`, reserved stock restored.
   Both transitions are guarded and idempotent (replayed webhooks are no-ops).

### Timeout / pending-order release

A background job in order-service sweeps every 60s for `pending` orders older than
the grace period (default 15 minutes, `ORDER_TIMEOUT_MINUTES`) and cancels them,
restoring stock through the same guarded compensation path — once.

## Safepay setup

Sandbox credentials are read from `.env` (never committed):

- `SAFEPAY_API_KEY` — merchant API key (`sec_…`).
- `SAFEPAY_MERCHANT_SECRET` — used as the `X-SFPY-MERCHANT-SECRET` header for
  session/passport calls. This is *not* the same value as the webhook secret.
- `SAFEPAY_WEBHOOK_SECRET` — HMAC key for `X-SFPY-SIGNATURE` on webhooks
  (HMAC-SHA512 over the raw body, hex-encoded; SHA256 accepted too).
- `SAFEPAY_ENV=sandbox`

Register the webhook endpoint as `https://<public>/webhooks/safepay`. In local
dev, expose `8085:8085` and `ngrok http <url> 8085` only for webhook testing,
then remove the port mapping. Payment-service and the webhook are intentionally
not gateway-exposed.

## Email via Resend

Order confirmations are sent by notification-service through [Resend](https://resend.com).
Configuration is read from `.env`:

- `RESEND_API_KEY` — account API key (`re_…`) with *Sending access*.
- `RESEND_FROM` — sender address (default `onboarding@resend.dev`).
- `EMAIL_MOCK` — when `true`, or when `RESEND_API_KEY` is unset, the service logs
  the email instead of sending. CI and `.env.example` default it to `true`.

On the free plan the default `onboarding@resend.dev` sender is test-only: Resend
delivers only to the address that owns the account and rejects reserved domains
like `example.com` with `422`. It also tends to land in spam, because the shared
sender domain carries no reputation. For real inbox delivery, verify your own
domain in Resend (SPF + DKIM, plus a DMARC record) and set
`RESEND_FROM=orders@yourdomain.com`. Sending is best-effort — a failed delivery
only logs a warning and never fails checkout.

## Observability

Every request carries a correlation id. The `request_id_middleware` in
`libs/common` reads an inbound `X-Request-Id`, reuses it when it is a valid UUID
and otherwise generates one, and echoes it on the response. order-service and
payment-service propagate it on their outbound calls and log it as `trace_id`,
so one checkout can be followed across services with a single id
(`docker compose logs | grep <id>`). `e2e/regression.sh` asserts that the same id
is echoed back and appears in both services' logs.

## Tests

`cargo test --workspace` runs 38 unit tests over the pure logic: Safepay webhook
signature verification and event classification, the admin status whitelist, the
pending-order timeout boundary, JWT `bearer_token` parsing, cart-item-to-stock
mapping, request-id resolution, and the email mock guard.

## Local development

```bash
cp .env.example .env       # fill in the secrets above
docker compose up -d       # network needed: builds via cargo-chef multi-stage
./e2e/seed.sh              # idempotent users + catalog (needs stack healthy)
./e2e/regression.sh        # 34-check end-to-end regression
```

`e2e/seed.sh` guarantees the users (`docker_test@example.com`, `regular@example.com`,
password `secret123`) and a 3-product catalog with stock ≥ 10. `e2e/regression.sh`
drives the whole flow through the gateway and injects *signed* synthetic webhooks
over the docker network — no real Safepay credentials needed to re-verify any of the
saga, cancel, timeout, or stock-compensation paths.

## CI

`.github/workflows/ci.yml` runs on every push/PR (doc-only `**.md` changes are
skipped):

1. Fast gates in one job sharing the `rust-cache` target dir: `cargo fmt --check`,
   `cargo clippy --all-targets --workspace -- -D warnings`, `cargo test --workspace`.
2. E2E (needs the gates): build the six service images in parallel with
   `docker buildx build --load` and a per-service `type=gha` layer cache, boot the
   stack with dummy `.env` values (`SAFEPAY_MOCK=true`, `EMAIL_MOCK=true`), seed,
   and run `e2e/regression.sh`.

Because the regression self-signs webhooks and email is mocked, the E2E stage is
fully hermetic — no real Safepay or Resend credentials required. `--load` is what
lets compose reuse the cached images instead of rebuilding them.

## Known limitations

- **No real refunds.** A `paid` order cannot be admin-cancelled (rejected
  `400`); stock is never released for a paid order without an actual refund.
  Handling real refunds means the Safepay refund API plus a new state branch.
- **Late payment after timeout-cancel is surfaced, not resolved.** If
  `payment.succeeded` arrives after the sweep cancelled the order, the customer
  has been charged for an order that will not be fulfilled. payment-service logs
  the event at `ERROR` and flags the payment row `paid_after_cancel` so it is
  visible and queryable; a manual refund is still required.
- **Email delivery depends on the sender domain.** Confirmations go through
  Resend, but on the free plan with the default `onboarding@resend.dev` sender
  mail is test-only (account owner only, `example.com` rejected) and likely to
  land in spam; inbox delivery needs a verified domain. Sends are best-effort and
  never block checkout.
- **Sandbox only.** Payments run against Safepay's sandbox (no live KYC or real
  money movement).
- **No frontend.** After hosted checkout, the customer lands on a static
  "payment received" page served by order-service
  (`/api/orders/payment-result/<id>/success|cancelled`, base via
  `PUBLIC_BASE_URL`).

See `implementationPlan.md` for the original design notes.