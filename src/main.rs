mod api;
mod config;
mod db;
mod gmail;
mod models;
mod openai;
mod service;
mod status;
mod track17;

use clap::Parser;
use config::{Cli, Command, Config};
use service::AppState;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("trackhound=info".parse()?))
        .init();
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Serve);
    let cfg = match command {
        Command::Migrate => Config::from_env_lenient_for_migrate()?,
        _ => Config::from_env()?,
    };
    config::ensure_sqlite_parent(&cfg.database_url)?;
    let pool = db::connect(&cfg.database_url).await?;
    db::migrate(&pool).await?;

    if matches!(command, Command::Migrate) {
        return Ok(());
    }

    let state = Arc::new(AppState {
        pool,
        gmail: gmail::GmailClient::new(cfg.clone()),
        classifier: openai::OpenAiClassifier::new(
            cfg.openai_api_key.clone(),
            cfg.openai_model.clone(),
            cfg.openai_url.clone(),
        ),
        track17: track17::Track17Client::new(cfg.track17_security_key.clone()),
        gmail_query: cfg.gmail_query.clone(),
    });

    match command {
        Command::Serve => {
            service::run_scheduler(
                state.clone(),
                cfg.gmail_scan_interval,
                cfg.track17_sync_interval,
            )
            .await;
            let listener = tokio::net::TcpListener::bind(cfg.bind_addr).await?;
            tracing::info!(addr = %cfg.bind_addr, "trackhound listening");
            axum::serve(listener, api::router(state))
                .with_graceful_shutdown(shutdown_signal())
                .await?;
        }
        Command::Scan => {
            println!(
                "{}",
                serde_json::to_string_pretty(&state.scan_gmail().await?)?
            );
        }
        Command::Sync => {
            println!(
                "{}",
                serde_json::to_string_pretty(&state.sync_track17().await?)?
            );
        }
        Command::Migrate => unreachable!(),
    }
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
