use crate::{
    db, gmail::GmailClient, models::*, openai::OpenAiClassifier, status::ShipmentStatus,
    track17::Track17Client,
};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::time::{interval, MissedTickBehavior};
use tracing::{error, info};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub gmail: GmailClient,
    pub classifier: OpenAiClassifier,
    pub track17: Track17Client,
    pub gmail_query: String,
}

impl AppState {
    pub async fn scan_gmail(&self) -> anyhow::Result<ScanSummary> {
        let ids = self.gmail.search_messages(&self.gmail_query, 50).await?;
        let mut summary = ScanSummary {
            messages_seen: ids.len(),
            messages_processed: 0,
            shipments_upserted: 0,
            orders_upserted: 0,
            registered_in_17track: 0,
        };
        for id in ids {
            if db::email_seen(&self.pool, &id).await? {
                continue;
            }
            let msg = self.gmail.get_message(&id).await?;
            let extraction = self.classifier.classify(&msg).await?;
            let classifier_json = serde_json::to_string(&extraction)?;
            for item in &extraction.shipments {
                if item.confidence < 0.70 || item.kind == ExtractedKind::Ignore {
                    continue;
                }
                let order_id = if item.order_number.is_some() {
                    summary.orders_upserted += 1;
                    Some(db::upsert_order(&self.pool, &msg, item).await?)
                } else {
                    None
                };
                match item.kind {
                    ExtractedKind::Tracking => {
                        if let Some(tn) = item.tracking_number.as_deref() {
                            let remark = item
                                .title
                                .as_deref()
                                .or(item.merchant.as_deref())
                                .or(item.order_number.as_deref());
                            let registered = match self.track17.register(tn, remark).await {
                                Ok(()) => {
                                    summary.registered_in_17track += 1;
                                    true
                                }
                                Err(e) => {
                                    error!(tracking = tn, error = %e, "17TRACK registration failed");
                                    false
                                }
                            };
                            db::upsert_shipment(
                                &self.pool,
                                &msg,
                                item,
                                order_id.as_deref(),
                                registered,
                            )
                            .await?;
                            summary.shipments_upserted += 1;
                        }
                    }
                    ExtractedKind::AmazonOrder => {}
                    ExtractedKind::StatusUpdate => {
                        if let Some(order_id) = order_id.as_deref() {
                            let status = item
                                .status
                                .as_deref()
                                .map(ShipmentStatus::from_text)
                                .unwrap_or(ShipmentStatus::Unknown);
                            db::mirror_order_status_if_single_shipment(
                                &self.pool,
                                order_id,
                                status,
                                item.reason.as_deref(),
                            )
                            .await?;
                        }
                    }
                    ExtractedKind::Ignore => {}
                }
            }
            db::record_email_seen(&self.pool, &msg, &classifier_json).await?;
            summary.messages_processed += 1;
        }
        Ok(summary)
    }

    pub async fn sync_track17(&self) -> anyhow::Result<SyncSummary> {
        let numbers = db::tracking_numbers_for_sync(&self.pool).await?;
        let mut summary = SyncSummary {
            checked: numbers.len(),
            updated: 0,
        };
        for chunk in numbers.chunks(40) {
            let statuses = self.track17.get_statuses(chunk).await?;
            for s in statuses {
                db::update_tracking_status(
                    &self.pool,
                    &s.number,
                    s.status,
                    s.event_text.as_deref(),
                    s.event_at.as_deref(),
                    &s.raw,
                )
                .await?;
                db::mark_registered(&self.pool, &s.number).await?;
                summary.updated += 1;
            }
        }
        Ok(summary)
    }
}

pub async fn run_scheduler(
    state: Arc<AppState>,
    gmail_every: std::time::Duration,
    track17_every: std::time::Duration,
) {
    let gmail_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(gmail_every);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            match gmail_state.scan_gmail().await {
                Ok(s) => info!(?s, "gmail scan complete"),
                Err(e) => error!(error=%e, "gmail scan failed"),
            }
        }
    });

    let sync_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(track17_every);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            match sync_state.sync_track17().await {
                Ok(s) => info!(?s, "17track sync complete"),
                Err(e) => error!(error=%e, "17track sync failed"),
            }
        }
    });
}
