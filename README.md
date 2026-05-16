# Trackhound

Trackhound is a small Rust service that watches Gmail for parcel/shipment emails, uses an OpenAI classifier to extract tracking/order data, registers real tracking numbers in 17TRACK, stores everything in SQLite, and exposes a simple HTTP API.

## Features

- Gmail API OAuth refresh-token flow; scans every 30 minutes by default.
- OpenAI structured JSON extraction from full email snippets.
- 17TRACK v1 API registration and hourly status sync.
- Amazon support: stores Amazon order numbers locally even when no tracking number is present.
- SQLite storage via `sqlx` migrations.
- One long-running service with internal scheduler.
- Helm chart with PVC-backed SQLite database.

## Multiple shipments per Amazon order

The data model treats an Amazon order as an `orders` row and each physical package as a `shipments` row linked by `order_id`.

- If an email has only an Amazon order number, Trackhound creates/updates the order and creates no package unless needed.
- If a later email for the same order contains tracking, Trackhound creates a shipment linked to that order.
- If multiple tracking numbers appear for the same Amazon order, each becomes a separate shipment under that one order.
- If Amazon sends status-only updates without tracking, Trackhound updates the order-level status and, when there is exactly one shipment for the order, mirrors the status to that shipment.

## Configuration

All settings are environment variables:

```bash
TRACKHOUND_DATABASE_URL=sqlite:///data/trackhound.sqlite
TRACKHOUND_BIND_ADDR=0.0.0.0:8080
TRACKHOUND_GMAIL_SCAN_INTERVAL_SECONDS=1800
TRACKHOUND_TRACK17_SYNC_INTERVAL_SECONDS=3600
TRACKHOUND_GMAIL_QUERY='newer_than:14d (shipment OR tracking OR package OR parcel OR delivery OR amazon OR dhl OR dpd OR ups OR fedex OR gls)'
TRACKHOUND_OPENAI_MODEL=gpt-4.1-nano
OPENAI_API_KEY=...
GMAIL_CLIENT_ID=...
GMAIL_CLIENT_SECRET=...
GMAIL_REFRESH_TOKEN=...
TRACK17_SECURITY_KEY=...
```

You can also mount the same OAuth values from the Gmail `credentials.json` / `token.json` files into env vars. For local development on Kirill's Hermes box:

```bash
eval "$(python3 scripts/hermes-secrets-to-env.py)"
export OPENAI_API_KEY=...
TRACKHOUND_DATABASE_URL=sqlite://./trackhound.sqlite cargo run -- serve
```

## API

- `GET /healthz`
- `GET /shipments`
- `GET /shipments/today`
- `GET /shipments/{id}`
- `POST /scan` — trigger Gmail scan now
- `POST /sync` — trigger 17TRACK sync now

## Local development

```bash
cargo test
TRACKHOUND_DATABASE_URL=sqlite://./trackhound.sqlite cargo run -- serve
```

## Helm

```bash
helm upgrade --install trackhound ./charts/trackhound \
  --namespace trackhound --create-namespace \
  --set secrets.openaiApiKey='...' \
  --set secrets.gmailClientId='...' \
  --set secrets.gmailClientSecret='...' \
  --set secrets.gmailRefreshToken='...' \
  --set secrets.track17SecurityKey='...'
```
