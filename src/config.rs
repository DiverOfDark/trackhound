use clap::{Parser, Subcommand};
use std::{env, net::SocketAddr, path::PathBuf, time::Duration};

#[derive(Debug, Parser)]
#[command(name = "trackhound", about = "Gmail + 17TRACK parcel tracker")]
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
    pub gmail_client_id: String,
    pub gmail_client_secret: String,
    pub gmail_refresh_token: String,
    pub gmail_token_uri: String,
    pub track17_security_key: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: env_or("TRACKHOUND_DATABASE_URL", "sqlite:///data/trackhound.sqlite"),
            bind_addr: env_or("TRACKHOUND_BIND_ADDR", "0.0.0.0:8080").parse()?,
            gmail_scan_interval: Duration::from_secs(env_or("TRACKHOUND_GMAIL_SCAN_INTERVAL_SECONDS", "1800").parse()?),
            track17_sync_interval: Duration::from_secs(env_or("TRACKHOUND_TRACK17_SYNC_INTERVAL_SECONDS", "3600").parse()?),
            gmail_query: env_or("TRACKHOUND_GMAIL_QUERY", "newer_than:14d (shipment OR tracking OR package OR parcel OR delivery OR amazon OR dhl OR dpd OR ups OR fedex OR gls)"),
            openai_api_key: env_required("OPENAI_API_KEY")?,
            openai_model: env_or("TRACKHOUND_OPENAI_MODEL", "gpt-4.1-nano"),
            gmail_client_id: env_required("GMAIL_CLIENT_ID")?,
            gmail_client_secret: env_required("GMAIL_CLIENT_SECRET")?,
            gmail_refresh_token: env_required("GMAIL_REFRESH_TOKEN")?,
            gmail_token_uri: env_or("GMAIL_TOKEN_URI", "https://oauth2.googleapis.com/token"),
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
            gmail_client_id: String::new(),
            gmail_client_secret: String::new(),
            gmail_refresh_token: String::new(),
            gmail_token_uri: env_or("GMAIL_TOKEN_URI", "https://oauth2.googleapis.com/token"),
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
