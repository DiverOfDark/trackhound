CREATE UNIQUE INDEX IF NOT EXISTS idx_shipments_amazon_placeholder_per_order
  ON shipments(order_id) WHERE source = 'amazon_placeholder';

INSERT INTO shipments (id, tracking_number, order_id, source, status, last_email_message_id, last_email_thread_id, last_email_subject, raw_last_event)
SELECT
    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(6))),
    NULL,
    o.id,
    'amazon_placeholder',
    o.status,
    o.last_email_message_id,
    o.last_email_thread_id,
    o.last_email_subject,
    o.raw_last_event
FROM orders o
WHERE NOT EXISTS (SELECT 1 FROM shipments s WHERE s.order_id = o.id);
