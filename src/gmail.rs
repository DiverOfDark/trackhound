use crate::{config::Config, models::EmailMessage};
use chrono::Utc;
use mailparse::MailHeaderMap;

#[derive(Clone)]
pub struct GmailClient {
    cfg: Config,
}

impl GmailClient {
    pub fn new(cfg: Config) -> Self {
        Self { cfg }
    }

    pub async fn search_messages(
        &self,
        query: &str,
        max_results: u32,
    ) -> anyhow::Result<Vec<String>> {
        let cfg = self.cfg.clone();
        let query = query.to_string();
        tokio::task::spawn_blocking(move || {
            let mut session = login_imap(&cfg)?;
            let criteria = format!("X-GM-RAW {}", quote_imap_string(&query));
            let uids = session.uid_search(criteria)?;
            let mut uids: Vec<u32> = uids.into_iter().collect();
            uids.sort_unstable_by(|a, b| b.cmp(a));
            let max_results = max_results as usize;
            let ids = uids
                .into_iter()
                .take(max_results)
                .map(|uid| format!("imap:{uid}"))
                .collect();
            let _ = session.logout();
            Ok(ids)
        })
        .await?
    }

    pub async fn get_message(&self, id: &str) -> anyhow::Result<EmailMessage> {
        let cfg = self.cfg.clone();
        let uid = id.strip_prefix("imap:").unwrap_or(id).to_string();
        tokio::task::spawn_blocking(move || {
            let mut session = login_imap(&cfg)?;
            let messages = session.uid_fetch(&uid, "RFC822")?;
            let message = messages
                .iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("IMAP message uid {uid} not found"))?;
            let body = message
                .body()
                .ok_or_else(|| anyhow::anyhow!("IMAP message uid {uid} has no RFC822 body"))?;
            let parsed = parse_raw_imap_email(&uid, body)?;
            let _ = session.logout();
            Ok(parsed)
        })
        .await?
    }
}

fn login_imap(cfg: &Config) -> anyhow::Result<imap::Session<imap::Connection>> {
    let client = imap::ClientBuilder::new(&cfg.gmail_imap_host, cfg.gmail_imap_port)
        .tls_kind(imap::TlsKind::Rust)
        .connect()?;
    let mut session = client
        .login(&cfg.gmail_imap_username, &cfg.gmail_imap_password)
        .map_err(|(e, _)| e)?;
    session.select(&cfg.gmail_imap_mailbox)?;
    Ok(session)
}

fn quote_imap_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn parse_raw_imap_email(uid: &str, raw: &[u8]) -> anyhow::Result<EmailMessage> {
    let parsed = mailparse::parse_mail(raw)?;
    let headers = parsed.get_headers();
    let subject = headers.get_first_value("Subject").unwrap_or_default();
    let from_addr = headers.get_first_value("From").unwrap_or_default();
    let message_id = headers
        .get_first_value("Message-ID")
        .unwrap_or_else(|| format!("imap:{uid}"));
    let body_text = extract_mailparse_text(&parsed)?;
    let snippet = body_text.chars().take(300).collect();
    let internal_date_ms = headers
        .get_first_value("Date")
        .and_then(|d| mailparse::dateparse(&d).ok())
        .map(|seconds| seconds * 1000)
        .unwrap_or_else(|| Utc::now().timestamp_millis());

    Ok(EmailMessage {
        id: format!("imap:{uid}"),
        thread_id: message_id,
        subject,
        from_addr,
        snippet,
        body_text,
        internal_date_ms,
    })
}

fn extract_mailparse_text(parsed: &mailparse::ParsedMail<'_>) -> anyhow::Result<String> {
    if parsed.subparts.is_empty() {
        let mimetype = parsed.ctype.mimetype.to_ascii_lowercase();
        if mimetype.starts_with("text/") || mimetype.is_empty() {
            return Ok(parsed.get_body()?);
        }
        return Ok(String::new());
    }

    let mut out = String::new();
    for part in &parsed.subparts {
        let text = extract_mailparse_text(part)?;
        if !text.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&text);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_gmail_raw_query_for_imap_search() {
        assert_eq!(
            quote_imap_string("newer_than:14d \"tracking\" \\ dhl"),
            "\"newer_than:14d \\\"tracking\\\" \\\\ dhl\""
        );
    }

    #[test]
    fn parses_raw_imap_email_into_email_message() {
        let raw = concat!(
            "From: DHL Paket <noreply@dhl.de>\r\n",
            "Subject: Your parcel is moving\r\n",
            "Date: Sat, 16 May 2026 20:30:00 +0000\r\n",
            "Message-ID: <imap-message-id@example.com>\r\n",
            "\r\n",
            "Tracking number: JJD000390012345678901\r\n"
        );

        let msg = parse_raw_imap_email("42", raw.as_bytes()).unwrap();

        assert_eq!(msg.id, "imap:42");
        assert_eq!(msg.thread_id, "<imap-message-id@example.com>");
        assert_eq!(msg.subject, "Your parcel is moving");
        assert_eq!(msg.from_addr, "DHL Paket <noreply@dhl.de>");
        assert!(msg.body_text.contains("JJD000390012345678901"));
        assert_eq!(msg.internal_date_ms, 1778963400000);
    }
}
