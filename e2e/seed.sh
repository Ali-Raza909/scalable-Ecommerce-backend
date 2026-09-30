#!/usr/bin/env bash
# Idempotent seed for a fresh (or existing) stack: guarantees the users and
# catalog the e2e regression needs. Safe to run repeatedly.
set -u

BASE="http://localhost:80"
CAT_ID="11111111-1111-1111-1111-111111111111"
ADMIN_EMAIL="docker_test@example.com"
REGULAR_EMAIL="regular@example.com"
PASSWORD="secret123"
ADMIN_PASSWORD="secret123"

say() { echo "seed: $1"; }

say "ensuring category '$CAT_ID' (Electronics)"
docker compose exec -T product-db psql -U product_svc -d products -c \
  "INSERT INTO categories (id, name) VALUES ('$CAT_ID', 'Electronics') ON CONFLICT (name) DO NOTHING" >/dev/null \
  || { echo "FATAL: could not reach product-db" >&2; exit 1; }

ensure_user() {
  local email="$1" pw="$2" name="$3"
  local code
  code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/users/register" \
    -H 'Content-Type: application/json' \
    -d "{\"email\":\"$email\",\"password\":\"$pw\",\"full_name\":\"$name\"}")
  if [ "$code" = "201" ]; then
    say "registered $email"
  elif [ "$code" = "409" ] || [ "$code" = "400" ]; then
    say "$email already exists (register returned $code)"
  else
    say "WARN: register $email returned $code (will continue)"
  fi
}

ensure_user "$ADMIN_EMAIL" "$ADMIN_PASSWORD" "Docker Admin"
ensure_user "$REGULAR_EMAIL" "$PASSWORD" "Regular User"

say "ensuring '$ADMIN_EMAIL' has role=admin (register always assigns 'user')"
docker compose exec -T user-db psql -U user_svc -d users -c \
  "UPDATE users SET role = 'admin' WHERE email = '$ADMIN_EMAIL'" >/dev/null \
  || { echo "FATAL: could not reach user-db" >&2; exit 1; }

ensure_product() {
  local name="$1" desc="$2" price="$3"
  local existing
  existing=$(curl -s "$BASE/api/products?limit=100" | python3 -c "
import sys, json
items = json.load(sys.stdin)['items']
hit = [p for p in items if p['name'] == '$name']
print(hit[0]['id'] + ' ' + str(hit[0]['stock_quantity']) if hit else '')
" 2>/dev/null)

  if [ -n "$existing" ]; then
    local id="${existing%% *}"
    local cur="${existing##* }"
    local delta=$((10 - cur))
    say "product '$name' exists (stock $cur) -> setting to 10"
    if [ "$delta" -ne 0 ]; then
      curl -s -o /dev/null -X PATCH "$BASE/api/products/$id/stock" \
        -H 'Content-Type: application/json' -d "{\"delta\":$delta}"
    fi
  else
    say "creating product '$name'"
    local code
    code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/products" \
      -H 'Content-Type: application/json' \
      -d "{\"category_id\":\"$CAT_ID\",\"name\":\"$name\",\"description\":\"$desc\",\"price_cents\":$price,\"stock_quantity\":10}")
    [ "$code" = "201" ] || { echo "FATAL: create product '$name' returned $code" >&2; exit 1; }
  fi
}

ensure_product "Wireless Mouse" "Seed e2e catalog" 1999
ensure_product "Keyboard" "Seed e2e catalog" 2999
ensure_product "Laptop" "Seed e2e catalog" 129999

say "done. users + 3 products (stock 10 each) present."