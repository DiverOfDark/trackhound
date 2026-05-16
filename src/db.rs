use crate::{models::*, status::ShipmentStatus};
use anyhow::Context;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;
use uuid::Uuid;

pub async fn connect(database_url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

pub async fn email_seen(pool: &SqlitePool, message_id: &str) -> anyhow::Result<bool> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM emails_seen WHERE message_id = ?")
        .bind(message_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

pub async fn record_email_seen(
    pool: &SqlitePool,
    msg: &EmailMessage,
    classifier_json: &str,
) -> anyhow::Result<()> {
    sqlx::query(r#"INSERT OR REPLACE INTO emails_seen(message_id, thread_id, subject, from_addr, internal_date_ms, classifier_json)
                  VALUES (?, ?, ?, ?, ?, ?)"#)
        .bind(&msg.id).bind(&msg.thread_id).bind(&msg.subject).bind(&msg.from_addr)
        .bind(msg.internal_date_ms).bind(classifier_json).execute(pool).await?;
    Ok(())
}

pub async fn upsert_order(
    pool: &SqlitePool,
    msg: &EmailMessage,
    item: &ExtractedShipment,
) -> anyhow::Result<String> {
    let order_number = item.order_number.as_ref().context("order_number missing")?;
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM orders WHERE source = ? AND order_number = ?")
            .bind("amazon")
            .bind(order_number)
            .fetch_optional(pool)
            .await?;
    let id = existing
        .map(|r| r.0)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let status = item
        .status
        .as_deref()
        .map(ShipmentStatus::from_text)
        .unwrap_or(ShipmentStatus::Detected)
        .as_str();
    sqlx::query(r#"INSERT INTO orders(id, source, order_number, merchant, status, last_email_message_id, last_email_thread_id, last_email_subject, raw_last_event)
                  VALUES (?, 'amazon', ?, ?, ?, ?, ?, ?, ?)
                  ON CONFLICT(source, order_number) DO UPDATE SET
                    merchant = COALESCE(excluded.merchant, orders.merchant),
                    status = excluded.status,
                    last_email_message_id = excluded.last_email_message_id,
                    last_email_thread_id = excluded.last_email_thread_id,
                    last_email_subject = excluded.last_email_subject,
                    raw_last_event = excluded.raw_last_event,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')"#)
        .bind(&id).bind(order_number).bind(&item.merchant).bind(status).bind(&msg.id)
        .bind(&msg.thread_id).bind(&msg.subject).bind(&item.reason).execute(pool).await?;
    Ok(id)
}

pub async fn upsert_shipment(
    pool: &SqlitePool,
    msg: &EmailMessage,
    item: &ExtractedShipment,
    order_id: Option<&str>,
    registered: bool,
) -> anyhow::Result<String> {
    let tracking = item.tracking_number.as_deref().map(normalize_tracking);
    let status = item
        .status
        .as_deref()
        .map(ShipmentStatus::from_text)
        .unwrap_or(if registered {
            ShipmentStatus::Registered
        } else {
            ShipmentStatus::Detected
        })
        .as_str();
    let id = Uuid::new_v4().to_string();
    if let Some(tracking) = tracking {
        sqlx::query(r#"INSERT INTO shipments(id, tracking_number, order_id, source, carrier, title, status, track17_registered, expected_delivery_date, last_event_text, raw_last_event, last_email_message_id, last_email_thread_id, last_email_subject)
                      VALUES (?, ?, ?, 'email', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                      ON CONFLICT(tracking_number) DO UPDATE SET
                        order_id = COALESCE(excluded.order_id, shipments.order_id),
                        carrier = COALESCE(excluded.carrier, shipments.carrier),
                        title = COALESCE(excluded.title, shipments.title),
                        status = excluded.status,
                        track17_registered = MAX(shipments.track17_registered, excluded.track17_registered),
                        expected_delivery_date = COALESCE(excluded.expected_delivery_date, shipments.expected_delivery_date),
                        last_event_text = COALESCE(excluded.last_event_text, shipments.last_event_text),
                        raw_last_event = COALESCE(excluded.raw_last_event, shipments.raw_last_event),
                        last_email_message_id = excluded.last_email_message_id,
                        last_email_thread_id = excluded.last_email_thread_id,
                        last_email_subject = excluded.last_email_subject,
                        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')"#)
            .bind(&id).bind(&tracking).bind(order_id).bind(&item.carrier).bind(&item.title)
            .bind(status).bind(if registered {1} else {0}).bind(&item.expected_delivery_date)
            .bind(&item.status).bind(&item.reason).bind(&msg.id).bind(&msg.thread_id).bind(&msg.subject)
            .execute(pool).await?;
        let row: (String,) = sqlx::query_as("SELECT id FROM shipments WHERE tracking_number = ?")
            .bind(&tracking)
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    } else {
        Ok(String::new())
    }
}

pub async fn upsert_amazon_placeholder(
    pool: &SqlitePool,
    msg: &EmailMessage,
    item: &ExtractedShipment,
    order_id: &str,
) -> anyhow::Result<String> {
    let status = item
        .status
        .as_deref()
        .map(ShipmentStatus::from_text)
        .unwrap_or(ShipmentStatus::Detected)
        .as_str();
    let id = Uuid::new_v4().to_string();
    sqlx::query(r#"INSERT INTO shipments(id, tracking_number, order_id, source, title, status, last_email_message_id, last_email_thread_id, last_email_subject, raw_last_event)
                  VALUES (?, NULL, ?, 'amazon_placeholder', ?, ?, ?, ?, ?, ?)
                  ON CONFLICT(order_id) WHERE source = 'amazon_placeholder' DO UPDATE SET
                    title = COALESCE(excluded.title, shipments.title),
                    status = excluded.status,
                    last_email_message_id = excluded.last_email_message_id,
                    last_email_thread_id = excluded.last_email_thread_id,
                    last_email_subject = excluded.last_email_subject,
                    raw_last_event = COALESCE(excluded.raw_last_event, shipments.raw_last_event),
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')"#)
        .bind(&id).bind(order_id).bind(&item.title).bind(status)
        .bind(&msg.id).bind(&msg.thread_id).bind(&msg.subject).bind(&item.reason)
        .execute(pool).await?;
    let row: (String,) = sqlx::query_as(
        "SELECT id FROM shipments WHERE order_id = ? AND source = 'amazon_placeholder'",
    )
    .bind(order_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn delete_amazon_placeholder(pool: &SqlitePool, order_id: &str) -> anyhow::Result<u64> {
    let result =
        sqlx::query("DELETE FROM shipments WHERE order_id = ? AND source = 'amazon_placeholder'")
            .bind(order_id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

pub async fn list_orders(pool: &SqlitePool) -> anyhow::Result<Vec<OrderRow>> {
    sqlx::query_as::<_, OrderRow>(r#"SELECT * FROM orders ORDER BY updated_at DESC"#)
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}

pub async fn order_by_id(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<OrderRow>> {
    sqlx::query_as::<_, OrderRow>(r#"SELECT * FROM orders WHERE id = ?"#)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(Into::into)
}

pub async fn shipments_for_order(
    pool: &SqlitePool,
    order_id: &str,
) -> anyhow::Result<Vec<ShipmentRow>> {
    sqlx::query_as::<_, ShipmentRow>(r#"SELECT s.*, o.order_number, o.merchant FROM shipments s LEFT JOIN orders o ON o.id = s.order_id WHERE s.order_id = ? ORDER BY s.updated_at DESC"#)
        .bind(order_id).fetch_all(pool).await.map_err(Into::into)
}

pub async fn mirror_order_status_if_single_shipment(
    pool: &SqlitePool,
    order_id: &str,
    status: ShipmentStatus,
    raw: Option<&str>,
) -> anyhow::Result<()> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM shipments WHERE order_id = ?")
        .bind(order_id)
        .fetch_one(pool)
        .await?;
    if count.0 == 1 {
        sqlx::query("UPDATE shipments SET status = ?, raw_last_event = COALESCE(?, raw_last_event), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE order_id = ?")
            .bind(status.as_str()).bind(raw).bind(order_id).execute(pool).await?;
    }
    Ok(())
}

pub async fn list_shipments(pool: &SqlitePool) -> anyhow::Result<Vec<ShipmentRow>> {
    sqlx::query_as::<_, ShipmentRow>(r#"SELECT s.*, o.order_number, o.merchant FROM shipments s LEFT JOIN orders o ON o.id = s.order_id ORDER BY s.updated_at DESC"#)
        .fetch_all(pool).await.map_err(Into::into)
}

pub async fn shipment_by_id(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<ShipmentRow>> {
    sqlx::query_as::<_, ShipmentRow>(r#"SELECT s.*, o.order_number, o.merchant FROM shipments s LEFT JOIN orders o ON o.id = s.order_id WHERE s.id = ?"#)
        .bind(id).fetch_optional(pool).await.map_err(Into::into)
}

pub async fn due_today(pool: &SqlitePool, today: &str) -> anyhow::Result<Vec<ShipmentRow>> {
    sqlx::query_as::<_, ShipmentRow>(r#"SELECT s.*, o.order_number, o.merchant FROM shipments s LEFT JOIN orders o ON o.id = s.order_id
        WHERE s.status = 'out_for_delivery' OR s.expected_delivery_date = ? ORDER BY s.updated_at DESC"#)
        .bind(today).fetch_all(pool).await.map_err(Into::into)
}

pub async fn tracking_numbers_for_sync(pool: &SqlitePool) -> anyhow::Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT tracking_number FROM shipments WHERE tracking_number IS NOT NULL AND status NOT IN ('delivered','failed')")
        .fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

pub async fn mark_registered(pool: &SqlitePool, tracking: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE shipments SET track17_registered = 1, status = CASE WHEN status='detected' THEN 'registered' ELSE status END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE tracking_number = ?")
        .bind(normalize_tracking(tracking)).execute(pool).await?;
    Ok(())
}

pub async fn update_tracking_status(
    pool: &SqlitePool,
    tracking: &str,
    status: ShipmentStatus,
    event_text: Option<&str>,
    event_at: Option<&str>,
    raw: &str,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE shipments SET status = ?, last_event_text = COALESCE(?, last_event_text), last_event_at = COALESCE(?, last_event_at), raw_last_event = ?, delivered_at = CASE WHEN ? = 'delivered' THEN COALESCE(delivered_at, strftime('%Y-%m-%dT%H:%M:%fZ','now')) ELSE delivered_at END, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE tracking_number = ?")
        .bind(status.as_str()).bind(event_text).bind(event_at).bind(raw).bind(status.as_str()).bind(normalize_tracking(tracking)).execute(pool).await?;
    Ok(())
}

pub fn normalize_tracking(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn amazon_order_can_have_multiple_shipments() {
        let pool = connect("sqlite::memory:").await.unwrap();
        migrate(&pool).await.unwrap();
        let msg = EmailMessage {
            id: "m1".into(),
            thread_id: "t1".into(),
            subject: "Amazon".into(),
            from_addr: "a@example.com".into(),
            snippet: "".into(),
            body_text: "".into(),
            internal_date_ms: 1,
        };
        let order = ExtractedShipment {
            kind: ExtractedKind::AmazonOrder,
            tracking_number: None,
            order_number: Some("123-1234567-1234567".into()),
            carrier: None,
            merchant: Some("Amazon".into()),
            status: Some("detected".into()),
            expected_delivery_date: None,
            title: None,
            confidence: 0.9,
            reason: None,
        };
        let order_id = upsert_order(&pool, &msg, &order).await.unwrap();
        for tn in ["AB123", "CD456"] {
            let item = ExtractedShipment {
                kind: ExtractedKind::Tracking,
                tracking_number: Some(tn.into()),
                order_number: order.order_number.clone(),
                carrier: None,
                merchant: Some("Amazon".into()),
                status: None,
                expected_delivery_date: None,
                title: None,
                confidence: 0.95,
                reason: None,
            };
            upsert_shipment(&pool, &msg, &item, Some(&order_id), true)
                .await
                .unwrap();
        }
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM shipments WHERE order_id = ?")
            .bind(&order_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 2);
    }

    #[test]
    fn normalizes_tracking_numbers() {
        assert_eq!(normalize_tracking(" rr 123 de "), "RR123DE");
    }

    #[tokio::test]
    async fn amazon_placeholder_is_idempotent_and_replaced_by_tracking() {
        let pool = connect("sqlite::memory:").await.unwrap();
        migrate(&pool).await.unwrap();
        let msg = EmailMessage {
            id: "m1".into(),
            thread_id: "t1".into(),
            subject: "Your Amazon order".into(),
            from_addr: "auto-confirm@amazon.com".into(),
            snippet: "".into(),
            body_text: "".into(),
            internal_date_ms: 1,
        };
        let order_item = ExtractedShipment {
            kind: ExtractedKind::AmazonOrder,
            tracking_number: None,
            order_number: Some("123-1234567-1234567".into()),
            carrier: None,
            merchant: Some("Amazon".into()),
            status: Some("detected".into()),
            expected_delivery_date: None,
            title: Some("Book".into()),
            confidence: 0.9,
            reason: None,
        };
        let order_id = upsert_order(&pool, &msg, &order_item).await.unwrap();

        let first = upsert_amazon_placeholder(&pool, &msg, &order_item, &order_id)
            .await
            .unwrap();
        let second = upsert_amazon_placeholder(&pool, &msg, &order_item, &order_id)
            .await
            .unwrap();
        assert_eq!(first, second, "placeholder must be idempotent per order");

        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM shipments WHERE order_id = ? AND source = 'amazon_placeholder'",
        )
        .bind(&order_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count.0, 1);

        let tracking_item = ExtractedShipment {
            kind: ExtractedKind::Tracking,
            tracking_number: Some("AB123".into()),
            order_number: order_item.order_number.clone(),
            carrier: None,
            merchant: Some("Amazon".into()),
            status: None,
            expected_delivery_date: None,
            title: None,
            confidence: 0.95,
            reason: None,
        };
        upsert_shipment(&pool, &msg, &tracking_item, Some(&order_id), true)
            .await
            .unwrap();
        let removed = delete_amazon_placeholder(&pool, &order_id).await.unwrap();
        assert_eq!(removed, 1);

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM shipments WHERE order_id = ?")
            .bind(&order_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 1, "only the real tracking shipment should remain");
    }
}
