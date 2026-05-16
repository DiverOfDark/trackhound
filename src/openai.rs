use crate::models::{EmailMessage, ExtractionResult};
use anyhow::Context;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{debug, error, trace};

#[derive(Clone)]
pub struct OpenAiClassifier {
    http: Client,
    api_key: String,
    model: String,
    openai_url: String,
}

impl OpenAiClassifier {
    pub fn new(api_key: String, model: String, openai_url: String) -> Self {
        Self {
            http: Client::new(),
            api_key,
            model,
            openai_url,
        }
    }

    pub async fn classify(&self, msg: &EmailMessage) -> anyhow::Result<ExtractionResult> {
        let content = format!(
            "From: {}\nSubject: {}\nSnippet: {}\nBody:\n{}",
            msg.from_addr,
            msg.subject,
            msg.snippet,
            truncate(&msg.body_text, 12000)
        );
        let body = json!({
            "model": self.model,
            "response_format": {"type": "json_object"},
            "messages": [
                {"role":"system", "content": SYSTEM_PROMPT},
                {"role":"user", "content": content}
            ]
        });
        debug!(
            url = %self.openai_url,
            model = %self.model,
            email_id = %msg.id,
            subject = %msg.subject,
            content_chars = content.chars().count(),
            "openai classify request"
        );
        trace!(payload = %body, "openai classify payload");
        let response = self
            .http
            .post(&self.openai_url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        let response_text = response.text().await?;
        debug!(%status, response_chars = response_text.len(), "openai classify response");
        if !status.is_success() {
            error!(
                %status,
                url = %self.openai_url,
                model = %self.model,
                email_id = %msg.id,
                body = %response_text,
                "openai classify HTTP error"
            );
            anyhow::bail!("OpenAI request failed: HTTP {status}: {response_text}");
        }
        trace!(body = %response_text, "openai classify response body");
        let value: Value = serde_json::from_str(&response_text)
            .with_context(|| format!("OpenAI response not JSON: {response_text}"))?;
        let text = value["choices"][0]["message"]["content"]
            .as_str()
            .with_context(|| format!("OpenAI response missing content: {response_text}"))?;
        debug!(extraction_chars = text.len(), "openai extraction payload");
        trace!(extraction = %text, "openai extraction body");
        parse_extraction_json(text)
    }
}

pub const SYSTEM_PROMPT: &str = r#"You extract parcel/shipment information from emails for a private tracking database.
Return ONLY JSON with this shape:
{"shipments":[{"kind":"tracking|amazon_order|status_update|ignore","tracking_number":null|string,"order_number":null|string,"carrier":null|string,"merchant":null|string,"status":null|string,"expected_delivery_date":null|string YYYY-MM-DD,"title":null|string,"confidence":0.0-1.0,"reason":null|string}]}
Rules:
- Use full email content, not regex only.
- Tracking numbers are parcel carrier tracking identifiers. Be conservative.
- Amazon emails often have only order numbers; output kind amazon_order with order_number only.
- If an Amazon order later has multiple tracking numbers, output each tracking number as its own tracking item with the same order_number.
- Status text should be simple: detected, registered, in_transit, out_for_delivery, delivered, failed, unknown.
- If unsure, output kind ignore or low confidence.
"#;

pub fn parse_extraction_json(text: &str) -> anyhow::Result<ExtractionResult> {
    let parsed: ExtractionResult = serde_json::from_str(text).context("invalid extraction JSON")?;
    Ok(parsed)
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    s.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_classifier_json() {
        let r = parse_extraction_json(r#"{"shipments":[{"kind":"tracking","tracking_number":"RR123DE","order_number":null,"carrier":"DHL","merchant":null,"status":"in_transit","expected_delivery_date":"2026-05-17","title":null,"confidence":0.98,"reason":"labeled tracking"}]}"#).unwrap();
        assert_eq!(r.shipments.len(), 1);
        assert_eq!(r.shipments[0].tracking_number.as_deref(), Some("RR123DE"));
    }

    #[tokio::test]
    async fn classifier_posts_to_custom_openai_url() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::POST)
                    .path("/custom/v1/chat/completions")
                    .header("authorization", "Bearer test-key");
                then.status(200).json_body(json!({
                    "choices": [{
                        "message": {
                            "content": "{\"shipments\":[]}"
                        }
                    }]
                }));
            })
            .await;
        let classifier = OpenAiClassifier::new(
            "test-key".to_string(),
            "test-model".to_string(),
            format!("{}/custom/v1/chat/completions", server.base_url()),
        );

        let result = classifier
            .classify(&EmailMessage {
                id: "1".to_string(),
                thread_id: "t1".to_string(),
                subject: "Shipment".to_string(),
                from_addr: "shop@example.com".to_string(),
                snippet: "Your package shipped".to_string(),
                body_text: "Tracking soon".to_string(),
                internal_date_ms: 0,
            })
            .await
            .unwrap();

        assert!(result.shipments.is_empty());
        mock.assert_async().await;
    }
}
