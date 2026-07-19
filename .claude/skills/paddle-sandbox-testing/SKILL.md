---
name: paddle-sandbox-testing
description: Set up and run a local end-to-end Paddle sandbox payment test for Tradstry — tunnels, env vars, the checkout domain, test cards, and verifying the webhook landed. Use this whenever the work touches Paddle, checkout, subscriptions, billing, plan upgrades, entitlements, webhooks, or the Plan & Usage UI, and whenever someone needs to see a real upgrade flow work locally. Also use it when a checkout says "Something went wrong", when a webhook never arrives, or when tearing the setup back down.
---

# Local Paddle sandbox testing

Getting a real checkout to complete locally needs four things lined up at once:
a public URL for Paddle's webhooks, a *domain-shaped* URL for the checkout page,
matching env vars on both sides, and CORS. Each one fails in a way that looks
like something else, which is why this file exists.

Ports are fixed: **backend 7899**, **frontend 3038** (`next dev -p 3038`).

## The shape of the thing

```
browser (lvh.me:3038) ──► Paddle.js overlay ──► Paddle
                                                  │
                                                  ▼  webhook
                                    ngrok static domain ──► localhost:7899
                                                                 │
                                              route stores ──► paddle_webhook_events
                                                                 │
                                              worker (5s) ──► users.plan = 'pro'
```

Two separate public surfaces. The **backend** needs a stable public URL because
Paddle's webhook destination is configured once and should not move. The
**frontend** needs a URL Paddle accepts as a "default payment link", which is a
different problem with a different solution.

## Setup

### 1. Env vars

`backend/.env`:

```
PADDLE_API_KEY=pdl_sdbx_apikey_...      # Developer tools → Authentication
PADDLE_WEBHOOK_SECRET=pdl_ntfset_...    # shown once, when the destination is created
PADDLE_PRICE_PRO=pri_...
PADDLE_PRICE_PRO_PLUS=pri_...
PADDLE_ENV=sandbox
CORS_ALLOWED_ORIGINS=http://localhost:3038,http://127.0.0.1:3038
```

`frontend/.env.local`:

```
NEXT_PUBLIC_PADDLE_CLIENT_TOKEN=test_...
NEXT_PUBLIC_PADDLE_ENV=sandbox
```

`NEXT_PUBLIC_PADDLE_ENV` is the one people forget. Without it Paddle.js talks to
**production**, which rejects a sandbox client token — and the overlay reports
this as a generic failure rather than an auth error.

Verify the price IDs actually map to the tier you think, because getting them
backwards silently sells Pro at the Pro Plus price:

```bash
set -a && source backend/.env; set +a
for p in "$PADDLE_PRICE_PRO" "$PADDLE_PRICE_PRO_PLUS"; do
  curl -s -H "Authorization: Bearer $PADDLE_API_KEY" \
    "https://sandbox-api.paddle.com/prices/$p?include=product" \
  | python3 -c "import json,sys; d=json.load(sys.stdin)['data']; \
print(d['id'], d.get('product',{}).get('name'), d['unit_price']['amount'], d['unit_price']['currency_code'])"
done
```

### 2. Backend tunnel (webhooks)

Use the **ngrok static domain**. It survives restarts, so the Paddle webhook
destination is configured once and never touched again:

```bash
ngrok http --url=<your-static>.ngrok-free.dev 7899
```

Confirm it reaches the backend before trusting it:

```bash
curl -s -o /dev/null -w "%{http_code}\n" \
  -H "ngrok-skip-browser-warning: 1" https://<your-static>.ngrok-free.dev/health
# want: 200
```

Paddle destination (Developer tools → Notifications) points at
`https://<your-static>.ngrok-free.dev/webhooks/paddle`. Subscribe to the
`subscription.*` events. Subscribing to everything works but buries the log in
`transaction.*` noise.

### 3. Frontend URL (checkout)

**Paddle rejects `localhost` as a default payment link.** Use `lvh.me` — a real
public domain whose DNS resolves to `127.0.0.1`, so the browser loads your local
dev server while Paddle sees a proper domain:

```
Paddle → Checkout → Checkout settings → Default payment link:
http://lvh.me:3038
```

Then add that origin to CORS and restart the backend:

```
CORS_ALLOWED_ORIGINS=http://localhost:3038,http://127.0.0.1:3038,http://lvh.me:3038
```

`CORS_ALLOWED_ORIGINS` is read **at boot**. Editing it without restarting means
every GraphQL call from the tunnelled origin fails, which looks like an auth or
network bug rather than config.

`localtest.me` works identically if `lvh.me` is ever unavailable.

**Browse the app at `http://lvh.me:3038`, not `localhost:3038`.** Paddle
requires the default payment link to match the domain the checkout opens from.

## Run the test

1. Start backend, ngrok, and `bun dev` — **restart `bun dev`** if you changed
   any `NEXT_PUBLIC_*` var, since Next only reads them at boot.
2. Open `http://lvh.me:3038` → sign in → avatar → **Account** → **Plan**.
3. Upgrade to Pro. Pay with:

```
Card    4242 4242 4242 4242    (success, no 3DS)
Expiry  any future date
CVV     any 3 digits
```

Other sandbox cards, when the unhappy paths matter:

| Card | Simulates |
|---|---|
| `4000 0038 0000 0446` | success behind a 3DS challenge |
| `4000 0000 0000 0002` | declined |
| `4000 0027 6000 3184` | succeeds, then fails on renewal — the way to reach `past_due` and the grace window honestly |

## Verify

The backend log is the fastest signal. A successful upgrade looks like:

```
[paddle] queued subscription.activated (evt_...)
[paddle] <user-id> -> pro (active) via evt_...
```

`subscription.created` and `subscription.activated` both applying is expected —
they carry identical values and the write is idempotent.

Then confirm the row actually changed:

```bash
docker exec tradstry-postgres psql -U tradstry -d postgres -c "
SET search_path TO tradstry_dev;
SELECT plan, subscription_status, paddle_subscription_id, current_period_end
FROM users WHERE id='<user-id>';"
```

And that nothing is stuck in the queue:

```bash
docker exec tradstry-postgres psql -U tradstry -d postgres -c "
SET search_path TO tradstry_dev;
SELECT event_type, processed_at IS NOT NULL AS processed, attempts, left(error,80) AS error
FROM paddle_webhook_events ORDER BY received_at DESC LIMIT 10;"
```

An event with `processed = false` and a non-null `error` is being retried, and
stops being claimed after `MAX_WEBHOOK_ATTEMPTS` (10). It stays in the table on
purpose so a permanently broken event is inspectable rather than silently gone.

### Testing without a browser

Paddle's simulator fires real, real-signed events at the destination — useful
for exercising the webhook path alone:

```bash
set -a && source backend/.env; set +a
curl -s -X POST -H "Authorization: Bearer $PADDLE_API_KEY" -H "Content-Type: application/json" \
  https://sandbox-api.paddle.com/simulations \
  -d '{"notification_setting_id":"ntfset_...","name":"E2E","type":"subscription.activated"}'
# then POST to /simulations/<id>/runs
```

Expect it to **fail** with `Unrecognised Paddle price_id` — the simulator sends a
canned sample price that isn't in your catalog, and refusing to guess a tier is
the correct behaviour. That failure is a passing test of the safety property.

## When it doesn't work

| Symptom | Cause |
|---|---|
| Checkout: "Something went wrong" | Default payment link unset, or doesn't match the domain you're browsing from |
| Checkout fails, sandbox token looks right | `NEXT_PUBLIC_PADDLE_ENV=sandbox` missing → Paddle.js hit production |
| GraphQL calls fail from the tunnel domain | Origin missing from `CORS_ALLOWED_ORIGINS`, or backend not restarted after editing it |
| No `[paddle] queued` line at all | Paddle never reached you — check the destination URL and that the tunnel returns 200 |
| `[paddle] queued` but no apply line | Worker rejected it — read `error` in `paddle_webhook_events` |
| Webhook 401 | `PADDLE_WEBHOOK_SECRET` doesn't match the destination's secret (it's shown once; recreate the destination to get a new one) |
| Wrong tier purchased | `PADDLE_PRICE_PRO` / `PADDLE_PRICE_PRO_PLUS` swapped |

### If you use cloudflared instead of ngrok

Quick tunnels work but are the more fragile path, and on at least one network
here they failed in a confusing way:

- **QUIC (UDP 7844) blocked** → the tunnel registers once, dies, and the hostname
  stops resolving. The browser reports `ERR_NAME_NOT_RESOLVED`, which reads like
  a DNS problem rather than a dead tunnel. Always pass `--protocol http2`:

  ```bash
  cloudflared tunnel --url http://localhost:3038 --protocol http2
  ```

- Read past the URL banner in the log. `Registered tunnel connection` can appear
  moments before a QUIC precheck failure that dooms it.
- Quick-tunnel hostnames change on every restart, so the Paddle setting and CORS
  need re-editing each time. `lvh.me` avoids this entirely — prefer it.
- Free ngrok allows **one** agent session and one endpoint. A second endpoint in
  the config seizes the static domain and starts serving the wrong app, which
  silently misroutes webhooks. Don't try to run both tunnels through ngrok.

The permanent fix for URL churn is moving the domain's DNS to Cloudflare and
using a **named** tunnel with stable hostnames. That's a production DNS change,
so do it deliberately rather than mid-test.

## Teardown

```bash
pkill -f cloudflared          # if used
# leave ngrok running if you want webhooks to keep working
```

Reset `CORS_ALLOWED_ORIGINS` to `http://localhost:3038,http://127.0.0.1:3038`.

Two things persist that no script cleans up: the **default payment link** in the
Paddle dashboard, and any **active sandbox subscription** on the test user.
Cancel the subscription (or reset the row) or the next run starts from Pro
instead of Free:

```bash
docker exec tradstry-postgres psql -U tradstry -d postgres -c "
SET search_path TO tradstry_dev;
UPDATE users SET plan='free', subscription_status=NULL, paddle_subscription_id=NULL,
  paddle_customer_id=NULL, subscription_updated_at=NULL, current_period_start=NULL,
  current_period_end=NULL, grace_until=NULL WHERE id='<user-id>';"
```

Sandbox test customers and prices can be archived via the API (`PATCH` with
`{"status":"archived"}`); they cannot be deleted.
