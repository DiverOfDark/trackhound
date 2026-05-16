# Trackhound

Trackhound is a small Rust service that watches Gmail via IMAP with a Google App Password, uses an OpenAI classifier to extract tracking/order data, registers real tracking numbers in 17TRACK, stores everything in SQLite, and exposes a simple HTTP API.

## Features

- Gmail IMAP app-password flow; scans every 30 minutes by default.
- Uses Gmail's `X-GM-RAW` IMAP extension, so `TRACKHOUND_GMAIL_QUERY` supports Gmail search operators such as `newer_than:14d`.
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
GMAIL_IMAP_HOST=imap.gmail.com
GMAIL_IMAP_PORT=993
GMAIL_IMAP_USERNAME=you@gmail.com
GMAIL_IMAP_PASSWORD='google-app-password-without-spaces'
GMAIL_IMAP_MAILBOX=INBOX
TRACK17_SECURITY_KEY=...
```

Create a Google App Password for the mailbox account and use that value as `GMAIL_IMAP_PASSWORD`. OAuth credentials are not used.

For local development on Kirill's Hermes box:

```bash
eval "$(python3 scripts/hermes-secrets-to-env.py)"
export OPENAI_API_KEY=...
export GMAIL_IMAP_USERNAME=you@gmail.com
export GMAIL_IMAP_PASSWORD='google-app-password-without-spaces'
TRACKHOUND_DATABASE_URL=sqlite://./trackhound.sqlite cargo run -- serve
```

## API

Interactive Scalar API documentation is served at:

- `GET /api-docs`
- `GET /api-docs/openapi.json` — raw OpenAPI document

Endpoints:

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
  --set config.gmailImapUsername='you@gmail.com' \
  --set secrets.gmailImapPassword='google-app-password' \
  --set secrets.track17SecurityKey='...'
```

To use an externally managed Secret, set `secrets.existingSecret` and inject the ExternalSecret manifest through `additionalObjects`:

```yaml
secrets:
  existingSecret: trackhound-secrets

additionalObjects:
  - apiVersion: external-secrets.io/v1
    kind: ExternalSecret
    metadata:
      name: trackhound-secrets
    spec:
      refreshInterval: 1h
      secretStoreRef:
        name: cluster-secrets
        kind: ClusterSecretStore
      target:
        name: trackhound-secrets
      data:
        - secretKey: OPENAI_API_KEY
          remoteRef:
            key: trackhound/openai-api-key
        - secretKey: GMAIL_IMAP_PASSWORD
          remoteRef:
            key: trackhound/gmail-imap-password
        - secretKey: TRACK17_SECURITY_KEY
          remoteRef:
            key: trackhound/track17-security-key
```
