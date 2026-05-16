use crate::status::ShipmentStatus;
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct Track17Client {
    http: Client,
    security_key: String,
}

#[derive(Debug, Clone)]
pub struct Track17Status {
    pub number: String,
    pub status: ShipmentStatus,
    pub event_text: Option<String>,
    pub event_at: Option<String>,
    pub raw: String,
}

impl Track17Client {
    pub fn new(security_key: String) -> Self {
        Self {
            http: Client::new(),
            security_key,
        }
    }

    pub async fn register(&self, number: &str, remark: Option<&str>) -> anyhow::Result<()> {
        let mut item = json!({"number": number});
        if let Some(remark) = remark {
            item["remark"] = json!(remark);
        }
        let value: Value = self
            .http
            .post("https://api.17track.net/track/v1/register")
            .header("17token", &self.security_key)
            .json(&json!([item]))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if contains_error_code(&value, -18019901) {
            return Ok(());
        }
        if value.get("code").and_then(|v| v.as_i64()).unwrap_or(0) != 0 {
            anyhow::bail!("17TRACK register failed: {value}");
        }
        Ok(())
    }

    pub async fn get_statuses(&self, numbers: &[String]) -> anyhow::Result<Vec<Track17Status>> {
        if numbers.is_empty() {
            return Ok(vec![]);
        }
        let payload: Vec<Value> = numbers.iter().map(|n| json!({"number": n})).collect();
        let value: Value = self
            .http
            .post("https://api.17track.net/track/v1/gettrackinfo")
            .header("17token", &self.security_key)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(parse_track17_statuses(&value))
    }
}

fn contains_error_code(value: &Value, code: i64) -> bool {
    match value {
        Value::Number(n) => n.as_i64() == Some(code),
        Value::Array(a) => a.iter().any(|v| contains_error_code(v, code)),
        Value::Object(o) => o.values().any(|v| contains_error_code(v, code)),
        _ => false,
    }
}

pub fn parse_track17_statuses(value: &Value) -> Vec<Track17Status> {
    let mut out = Vec::new();
    if let Some(accepted) = value.pointer("/data/accepted").and_then(|v| v.as_array()) {
        for item in accepted {
            let number = item
                .get("number")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    item.pointer("/track_info/tracking/number")
                        .and_then(|v| v.as_str())
                })
                .unwrap_or_default()
                .to_string();
            if number.is_empty() {
                continue;
            }
            let code = item
                .pointer("/track_info/latest_status/status")
                .and_then(|v| v.as_i64())
                .or_else(|| {
                    item.pointer("/track_info/tracking/providers/0/provider_status")
                        .and_then(|v| v.as_i64())
                });
            let latest = item.pointer("/track_info/latest_event");
            let event_text = latest
                .and_then(|e| e.get("description"))
                .and_then(|v| v.as_str())
                .or_else(|| latest.and_then(|e| e.get("event")).and_then(|v| v.as_str()))
                .map(|s| s.to_string());
            let event_at = latest
                .and_then(|e| e.get("time_iso"))
                .and_then(|v| v.as_str())
                .or_else(|| {
                    latest
                        .and_then(|e| e.get("time_utc"))
                        .and_then(|v| v.as_str())
                })
                .map(|s| s.to_string());
            let status = ShipmentStatus::from_17track_status(code, event_text.as_deref());
            out.push(Track17Status {
                number,
                status,
                event_text,
                event_at,
                raw: item.to_string(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_accepted_status() {
        let v = json!({"data":{"accepted":[{"number":"RR123DE","track_info":{"latest_status":{"status":40},"latest_event":{"description":"Delivered","time_iso":"2026-05-16T10:00:00Z"}}}]}});
        let statuses = parse_track17_statuses(&v);
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].status, ShipmentStatus::Delivered);
    }
}
