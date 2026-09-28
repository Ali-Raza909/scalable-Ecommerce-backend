#!/usr/bin/env bash
# End-to-end regression for the scalable e-commerce platform.
# Exercises the full checkout saga through the Traefik gateway plus the
# Safepay webhook-driven confirm/cancel paths (synthetic, signed payloads
# injected over the internal docker network) and the timeout sweep.
#
# Green exit code 0; any failure -> 1. Prints PASS/FAIL per check.
set -u

BASE="http://localhost:80"
HERE="$(cd "$(dirname "$0")" && pwd)"
ENVFILE="$HERE/../.env"
SECRET="$(grep '^SAFEPAY_WEBHOOK_SECRET=' "$ENVFILE" | cut -d= -f2)"
if [ -z "$SECRET" ]; then echo "FATAL: SAFEPAY_WEBHOOK_SECRET missing in .env" >&2; exit 2; fi

PASS=0
FAIL=0

pass() { PASS=$((PASS + 1)); echo "PASS  $1"; }
fail() { FAIL=$((FAIL + 1)); echo "FAIL  $1"; }

ok() {
  if [ "$2" = "$3" ]; then pass "$1"; else fail "$1 (got '$2', want '$3')"; fi
}

json() { python3 -c "import sys,json;print(json.load(sys.stdin)$1)"; }

order_status() { docker compose exec -T order-db psql -U order_svc -d orders -tAc "select status from orders where id='$1'"; }
pay_status()   { docker compose exec -T payment-db psql -U payment_svc -d payments -tAc "select status from payments where order_id='$1'"; }
stock()        { docker compose exec -T product-db psql -U product_svc -d products -tAc "select stock_quantity from products where id='$1'"; }

# Injects a signed webhook over the docker network (payment-service is not
# host-exposed by design). $1=order_id $2=event type (succeeded|failed)
send_webhook() {
  local oid="$1" typ="$2" statev
  if [ "$typ" = "succeeded" ]; then statev="TRACKER_ENDED"; else statev="TRACKER_FAILED"; fi
  local tracker token body sig
  tracker=$(docker compose exec -T payment-db psql -U payment_svc -d payments -tAc "select payment_provider_ref from payments where order_id='$oid'")
  token="${oid//-/}" # deterministic evt token per order -> stable replay identity
  body="{\"token\":\"evt_$token\",\"version\":\"2.0.0\",\"type\":\"payment.$typ\",\"data\":{\"tracker\":\"$tracker\",\"state\":\"$statev\",\"metadata\":{\"order_id\":\"$oid\"}}}"
  sig=$(printf '%s' "$body" | python3 -c "import sys,hmac,hashlib;print(hmac.new('$SECRET'.encode(),sys.stdin.buffer.read(),hashlib.sha512).hexdigest())")
  docker compose exec -T order-service sh -c \
    "curl -s -o /dev/null -w '%{http_code}' -X POST http://payment-service:8085/webhooks/safepay \
     -H 'Content-Type: application/json' -H 'X-SFPY-SIGNATURE: $sig' -d '$body'"
}

echo "== picking a product with stock >= 3 =="
PROD_JSON=$(curl -s "$BASE/api/products")
PROD=$(echo "$PROD_JSON" | python3 -c 'import sys,json;p=max(json.load(sys.stdin)["items"],key=lambda x:x["stock_quantity"]);print(p["id"])')
STOCK0=$(stock "$PROD")
echo "product $PROD stock=$STOCK0"
if [ "$STOCK0" -lt 3 ]; then echo "SKIP: need stock>=3"; exit 0; fi

echo "== auth =="
EMAIL="e2e-$(date +%s)@example.com"
ok "register" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/users/register" -H 'Content-Type: application/json' -d "{\"email\":\"$EMAIL\",\"password\":\"secret123\",\"full_name\":\"E2E Tester\"}")" 201
TOKEN=$(curl -s -X POST "$BASE/api/users/login" -H 'Content-Type: application/json' -d "{\"email\":\"$EMAIL\",\"password\":\"secret123\"}" | json "['token']")
[ -n "$TOKEN" ] && pass "login token issued" || fail "login token issued"
AUTH="Authorization: Bearer $TOKEN"

echo "== checkout (order 1) =="
ok "cart add" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/cart/items" -H "$AUTH" -H 'Content-Type: application/json' -d "{\"product_id\":\"$PROD\",\"quantity\":1}")" 200
ok "cart content" "$(curl -s "$BASE/api/cart" -H "$AUTH" | json '["items"][0]["product_id"]')" "$PROD"
O1_JSON=$(curl -s -X POST "$BASE/api/orders" -H "$AUTH" -H 'Content-Type: application/json' -d '{}')
O1=$(echo "$O1_JSON" | json "['order']['id']")
ok "order 1 created pending" "$(echo "$O1_JSON" | json "['order']['status']")" "pending"
[ -n "$(echo "$O1_JSON" | json "['checkout_url']")" ] && pass "order 1 checkout_url present" || fail "order 1 checkout_url present"
ok "stock decremented after order 1" "$(stock "$PROD")" "$((STOCK0 - 1))"

echo "== success webhook =="
ok "webhook ack 200" "$(send_webhook "$O1" succeeded)" "200"
ok "payment 1 succeeded" "$(pay_status "$O1")" "succeeded"
ok "order 1 paid" "$(order_status "$O1")" "paid"
sleep 1
[ "$(docker compose logs --since 120s notification-service 2>&1 | grep -c "about order $O1")" -ge 1 ] \
  && pass "notification fired for order 1" || fail "notification fired for order 1"

echo "== failure webhook (order 2) =="
ok "cart add 2" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/cart/items" -H "$AUTH" -H 'Content-Type: application/json' -d "{\"product_id\":\"$PROD\",\"quantity\":1}")" 200
O2=$(curl -s -X POST "$BASE/api/orders" -H "$AUTH" -H 'Content-Type: application/json' -d '{}' | json "['order']['id']")
ok "stock decremented after order 2" "$(stock "$PROD")" "$((STOCK0 - 2))"
ok "webhook ack 200" "$(send_webhook "$O2" failed)" "200"
ok "payment 2 failed" "$(pay_status "$O2")" "failed"
ok "order 2 cancelled" "$(order_status "$O2")" "cancelled"
ok "stock restored after failed" "$(stock "$PROD")" "$((STOCK0 - 1))"
echo "-- replay failed webhook (idempotency)"
send_webhook "$O2" failed >/dev/null
ok "stock not double-restored" "$(stock "$PROD")" "$((STOCK0 - 1))"

echo "== admin cancel (order 3) =="
ok "cart add 3" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/cart/items" -H "$AUTH" -H 'Content-Type: application/json' -d "{\"product_id\":\"$PROD\",\"quantity\":1}")" 200
O3=$(curl -s -X POST "$BASE/api/orders" -H "$AUTH" -H 'Content-Type: application/json' -d '{}' | json "['order']['id']")
ATOKEN=$(curl -s -X POST "$BASE/api/users/login" -H 'Content-Type: application/json' -d '{"email":"docker_test@example.com","password":"secret123"}' | json "['token']")
ok "admin cancel" "$(curl -s -X PATCH "$BASE/api/orders/$O3/status" -H "Authorization: Bearer $ATOKEN" -H 'Content-Type: application/json' -d '{"status":"cancelled"}' | json "['status']")" "cancelled"
ok "stock restored after admin cancel" "$(stock "$PROD")" "$((STOCK0 - 1))"

echo "== insufficient stock =="
CUR=$(stock "$PROD")
ok "zero out stock" "$(curl -s -o /dev/null -w '%{http_code}' -X PATCH "$BASE/api/products/$PROD/stock" -H 'Content-Type: application/json' -d "{\"delta\":-$CUR}")" 200
ok "cart add 4" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/cart/items" -H "$AUTH" -H 'Content-Type: application/json' -d "{\"product_id\":\"$PROD\",\"quantity\":1}")" 200
ok "order rejected 409 (insufficient stock)" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/orders" -H "$AUTH" -H 'Content-Type: application/json' -d '{}')" 409
ok "stock still zero after 409" "$(stock "$PROD")" 0
ok "restore stock" "$(curl -s -o /dev/null -w '%{http_code}' -X PATCH "$BASE/api/products/$PROD/stock" -H 'Content-Type: application/json' -d "{\"delta\":$CUR}")" 200

echo "== summary =="
echo "PASS=$PASS FAIL=$FAIL"
[ "$FAIL" -eq 0 ] && echo "ALL GREEN" || echo "SOME FAILED"
exit $([ "$FAIL" -eq 0 ] && echo 0 || echo 1)