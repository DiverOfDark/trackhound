use crate::{config::Config, models::EmailMessage};
use base64::{engine::general_purpose::URL_SAFE, Engine};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone)]
pub struct GmailClient {
    http: Client,
    cfg: Config,
}

impl GmailClient {
    pub fn new(cfg: Config) -> Self {
        Self {
            http: Client::new(),
            cfg,
        }
    }

    async fn access_token(&self) -> anyhow::Result<String> {
        #[derive(Deserialize)]
        struct TokenResp {
            access_token: String,
        }
        let params = [
            ("client_id", self.cfg.gmail_client_id.as_str()),
            ("client_secret", self.cfg.gmail_client_secret.as_str()),
            ("refresh_token", self.cfg.gmail_refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ];
        let resp = self
            .http
            .post(&self.cfg.gmail_token_uri)
            .form(&params)
            .send()
            .await?
            .error_for_status()?
            .json::<TokenResp>()
            .await?;
        Ok(resp.access_token)
    }

    pub async fn search_messages(
        &self,
        query: &str,
        max_results: u32,
    ) -> anyhow::Result<Vec<String>> {
        #[derive(Deserialize)]
        struct ListResp {
            messages: Option<Vec<MessageId>>,
        }
        #[derive(Deserialize)]
        struct MessageId {
            id: String,
        }
        let token = self.access_token().await?;
        let resp = self
            .http
            .get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
            .bearer_auth(token)
            .query(&[("q", query), ("maxResults", &max_results.to_string())])
            .send()
            .await?
            .error_for_status()?
            .json::<ListResp>()
            .await?;
        Ok(resp
            .messages
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.id)
            .collect())
    }

    pub async fn get_message(&self, id: &str) -> anyhow::Result<EmailMessage> {
        let token = self.access_token().await?;
        let raw = self
            .http
            .get(format!(
                "https://gmail.googleapis.com/gmail/v1/users/me/messages/{id}"
            ))
            .bearer_auth(token)
            .query(&[("format", "full")])
            .send()
            .await?
            .error_for_status()?
            .json::<GmailMessage>()
            .await?;
        Ok(raw.into_email_message())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GmailMessage {
    id: String,
    thread_id: String,
    snippet: Option<String>,
    internal_date: Option<String>,
    payload: Option<GmailPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GmailPayload {
    mime_type: Option<String>,
    headers: Option<Vec<GmailHeader>>,
    body: Option<GmailBody>,
    parts: Option<Vec<GmailPayload>>,
}

#[derive(Debug, Deserialize)]
struct GmailHeader {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct GmailBody {
    data: Option<String>,
}

impl GmailMessage {
    fn into_email_message(self) -> EmailMessage {
        let headers = header_map(self.payload.as_ref());
        let subject = headers.get("subject").cloned().unwrap_or_default();
        let from_addr = headers.get("from").cloned().unwrap_or_default();
        let body_text = self.payload.as_ref().map(extract_text).unwrap_or_default();
        EmailMessage {
            id: self.id,
            thread_id: self.thread_id,
            subject,
            from_addr,
            snippet: self.snippet.unwrap_or_default(),
            body_text,
            internal_date_ms: self
                .internal_date
                .and_then(|d| d.parse().ok())
                .unwrap_or_default(),
        }
    }
}

fn header_map(payload: Option<&GmailPayload>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(headers) = payload.and_then(|p| p.headers.as_ref()) {
        for h in headers {
            map.insert(h.name.to_lowercase(), h.value.clone());
        }
    }
    map
}

fn extract_text(payload: &GmailPayload) -> String {
    let mut out = String::new();
    if payload
        .mime_type
        .as_deref()
        .unwrap_or("")
        .starts_with("text/")
    {
        if let Some(data) = payload.body.as_ref().and_then(|b| b.data.as_ref()) {
            let mut padded = data.clone();
            while padded.len() % 4 != 0 {
                padded.push('=');
            }
            if let Ok(bytes) = URL_SAFE.decode(padded) {
                out.push_str(&String::from_utf8_lossy(&bytes));
            }
        }
    }
    if let Some(parts) = &payload.parts {
        for p in parts {
            out.push('\n');
            out.push_str(&extract_text(p));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_base64url_text_part() {
        let payload = GmailPayload {
            mime_type: Some("text/plain".into()),
            headers: None,
            body: Some(GmailBody {
                data: Some("SGVsbG8tdHJhY2tpbmc".into()),
            }),
            parts: None,
        };
        assert_eq!(extract_text(&payload), "Hello-tracking");
    }
}
