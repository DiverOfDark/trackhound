use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ShipmentRow {
    pub id: String,
    pub tracking_number: Option<String>,
    pub order_id: Option<String>,
    pub source: String,
    pub carrier: Option<String>,
    pub title: Option<String>,
    pub status: String,
    pub track17_registered: i64,
    pub expected_delivery_date: Option<String>,
    pub delivered_at: Option<String>,
    pub last_event_at: Option<String>,
    pub last_event_text: Option<String>,
    pub raw_last_event: Option<String>,
    pub last_email_message_id: Option<String>,
    pub last_email_thread_id: Option<String>,
    pub last_email_subject: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub order_number: Option<String>,
    pub merchant: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub id: String,
    pub thread_id: String,
    pub subject: String,
    pub from_addr: String,
    pub snippet: String,
    pub body_text: String,
    pub internal_date_ms: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractedKind {
    Tracking,
    AmazonOrder,
    StatusUpdate,
    Ignore,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractedShipment {
    pub kind: ExtractedKind,
    pub tracking_number: Option<String>,
    pub order_number: Option<String>,
    pub carrier: Option<String>,
    pub merchant: Option<String>,
    pub status: Option<String>,
    pub expected_delivery_date: Option<String>,
    pub title: Option<String>,
    pub confidence: f32,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractionResult {
    pub shipments: Vec<ExtractedShipment>,
}

#[derive(Debug, Serialize)]
pub struct ScanSummary {
    pub messages_seen: usize,
    pub messages_processed: usize,
    pub shipments_upserted: usize,
    pub orders_upserted: usize,
    pub registered_in_17track: usize,
}

#[derive(Debug, Serialize)]
pub struct SyncSummary {
    pub checked: usize,
    pub updated: usize,
}

#[derive(Debug, Serialize)]
pub struct Health {
    pub ok: bool,
}

pub fn today_utc_date() -> String {
    Utc::now().date_naive().to_string()
}
