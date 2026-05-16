use clap::{Parser, Subcommand};
#[cfg(test)]
use std::sync::Mutex;
use std::{env, net::SocketAddr, path::PathBuf, time::Duration};

#[derive(Debug, Parser)]
#[command(name = "trackhound", about = "Gmail IMAP + 17TRACK parcel tracker")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand, Clone, Copy)]
pub enum Command {
    Serve,
    Scan,
    Sync,
    Migrate,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub gmail_scan_interval: Duration,
    pub track17_sync_interval: Duration,
    pub gmail_query: String,
    pub openai_api_key: String,
    pub openai_model: String,
    pub openai_url: String,
    pub gmail_imap_host: String,
    pub gmail_imap_port: u16,
    pub gmail_imap_username: String,
    pub gmail_imap_password: String,
    pub gmail_imap_mailbox: String,
    pub track17_security_key: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: env_or("TRACKHOUND_DATABASE_URL", "sqlite:///data/trackhound.sqlite"),
            bind_addr: env_or("TRACKHOUND_BIND_ADDR", "0.0.0.0:8080").parse()?,
            gmail_scan_interval: Duration::from_secs(
                env_or("TRACKHOUND_GMAIL_SCAN_INTERVAL_SECONDS", "1800").parse()?,
            ),
            track17_sync_interval: Duration::from_secs(
                env_or("TRACKHOUND_TRACK17_SYNC_INTERVAL_SECONDS", "3600").parse()?,
            ),
            gmail_query: env_or("TRACKHOUND_GMAIL_QUERY", "newer_than:14d (shipment OR tracking OR package OR parcel OR delivery OR amazon OR dhl OR dpd OR ups OR fedex OR gls)"),
            openai_api_key: env_required("OPENAI_API_KEY")?,
            openai_model: env_or("TRACKHOUND_OPENAI_MODEL", "gpt-4.1-nano"),
            openai_url: env_or(
                "TRACKHOUND_OPENAI_URL",
                "https://api.openai.com/v1/chat/completions",
            ),
            gmail_imap_host: env_or("GMAIL_IMAP_HOST", "imap.gmail.com"),
            gmail_imap_port: env_or("GMAIL_IMAP_PORT", "993").parse()?,
            gmail_imap_username: env_required("GMAIL_IMAP_USERNAME")?,
            gmail_imap_password: env_required("GMAIL_IMAP_PASSWORD")?,
            gmail_imap_mailbox: env_or("GMAIL_IMAP_MAILBOX", "INBOX"),
            track17_security_key: env_required("TRACK17_SECURITY_KEY")?,
        })
    }

    pub fn from_env_lenient_for_migrate() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: env_or(
                "TRACKHOUND_DATABASE_URL",
                "sqlite:///data/trackhound.sqlite",
            ),
            bind_addr: env_or("TRACKHOUND_BIND_ADDR", "0.0.0.0:8080").parse()?,
            gmail_scan_interval: Duration::from_secs(1800),
            track17_sync_interval: Duration::from_secs(3600),
            gmail_query: String::new(),
            openai_api_key: String::new(),
            openai_model: env_or("TRACKHOUND_OPENAI_MODEL", "gpt-4.1-nano"),
            openai_url: env_or(
                "TRACKHOUND_OPENAI_URL",
                "https://api.openai.com/v1/chat/completions",
            ),
            gmail_imap_host: env_or("GMAIL_IMAP_HOST", "imap.gmail.com"),
            gmail_imap_port: env_or("GMAIL_IMAP_PORT", "993").parse()?,
            gmail_imap_username: String::new(),
            gmail_imap_password: String::new(),
            gmail_imap_mailbox: env_or("GMAIL_IMAP_MAILBOX", "INBOX"),
            track17_security_key: String::new(),
        })
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_required(key: &str) -> anyhow::Result<String> {
    env::var(key).map_err(|_| anyhow::anyhow!("missing required env var {key}"))
}

pub fn ensure_sqlite_parent(database_url: &str) -> anyhow::Result<()> {
    let Some(path) = database_url.strip_prefix("sqlite://") else {
        return Ok(());
    };
    if path == ":memory:" {
        return Ok(());
    }
    let p = PathBuf::from(path);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn config_requires_imap_credentials_but_not_oauth_credentials() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_trackhound_env();
        env::set_var("OPENAI_API_KEY", "openai");
        env::set_var("GMAIL_IMAP_USERNAME", "kirill@example.com");
        env::set_var("GMAIL_IMAP_PASSWORD", "app-password");
        env::set_var("TRACK17_SECURITY_KEY", "track17");

        let cfg = Config::from_env().unwrap();

        assert_eq!(cfg.gmail_imap_host, "imap.gmail.com");
        assert_eq!(cfg.gmail_imap_port, 993);
        assert_eq!(cfg.gmail_imap_username, "kirill@example.com");
        assert_eq!(cfg.openai_url, "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn config_accepts_custom_openai_url() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_trackhound_env();
        env::set_var("OPENAI_API_KEY", "openai");
        env::set_var(
            "TRACKHOUND_OPENAI_URL",
            "http://openai-compatible.local/v1/chat/completions",
        );
        env::set_var("GMAIL_IMAP_USERNAME", "kirill@example.com");
        env::set_var("GMAIL_IMAP_PASSWORD", "app-password");
        env::set_var("TRACK17_SECURITY_KEY", "track17");

        let cfg = Config::from_env().unwrap();

        assert_eq!(
            cfg.openai_url,
            "http://openai-compatible.local/v1/chat/completions"
        );
    }

    #[test]
    fn config_rejects_missing_imap_password() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_trackhound_env();
        env::set_var("OPENAI_API_KEY", "openai");
        env::set_var("GMAIL_IMAP_USERNAME", "kirill@example.com");
        env::set_var("TRACK17_SECURITY_KEY", "track17");

        let err = Config::from_env().unwrap_err().to_string();

        assert!(err.contains("GMAIL_IMAP_PASSWORD"));
    }

    fn clear_trackhound_env() {
        for key in [
            "TRACKHOUND_DATABASE_URL",
            "TRACKHOUND_BIND_ADDR",
            "TRACKHOUND_GMAIL_SCAN_INTERVAL_SECONDS",
            "TRACKHOUND_TRACK17_SYNC_INTERVAL_SECONDS",
            "TRACKHOUND_GMAIL_QUERY",
            "TRACKHOUND_OPENAI_MODEL",
            "TRACKHOUND_OPENAI_URL",
            "OPENAI_API_KEY",
            "GMAIL_IMAP_HOST",
            "GMAIL_IMAP_PORT",
            "GMAIL_IMAP_USERNAME",
            "GMAIL_IMAP_PASSWORD",
            "GMAIL_IMAP_MAILBOX",
            "TRACK17_SECURITY_KEY",
        ] {
            env::remove_var(key);
        }
    }
}
