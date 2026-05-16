CREATE TABLE IF NOT EXISTS orders (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    order_number TEXT NOT NULL,
    merchant TEXT,
    status TEXT NOT NULL DEFAULT 'detected',
    last_email_message_id TEXT,
    last_email_thread_id TEXT,
    last_email_subject TEXT,
    raw_last_event TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(source, order_number)
);

CREATE TABLE IF NOT EXISTS shipments (
    id TEXT PRIMARY KEY,
    tracking_number TEXT UNIQUE,
    order_id TEXT REFERENCES orders(id) ON DELETE SET NULL,
    source TEXT NOT NULL DEFAULT 'email',
    carrier TEXT,
    title TEXT,
    status TEXT NOT NULL DEFAULT 'detected',
    track17_registered INTEGER NOT NULL DEFAULT 0,
    expected_delivery_date TEXT,
    delivered_at TEXT,
    last_event_at TEXT,
    last_event_text TEXT,
    raw_last_event TEXT,
    last_email_message_id TEXT,
    last_email_thread_id TEXT,
    last_email_subject TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS emails_seen (
    message_id TEXT PRIMARY KEY,
    thread_id TEXT,
    subject TEXT,
    from_addr TEXT,
    internal_date_ms INTEGER,
    processed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    classifier_json TEXT
);

CREATE TABLE IF NOT EXISTS sync_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX IF NOT EXISTS idx_shipments_status ON shipments(status);
CREATE INDEX IF NOT EXISTS idx_shipments_expected_delivery ON shipments(expected_delivery_date);
CREATE INDEX IF NOT EXISTS idx_shipments_order_id ON shipments(order_id);
CREATE INDEX IF NOT EXISTS idx_orders_order_number ON orders(order_number);
