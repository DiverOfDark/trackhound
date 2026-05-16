use crate::{db, models::Health, service::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/shipments", get(list_shipments))
        .route("/shipments/today", get(today))
        .route("/shipments/:id", get(get_shipment))
        .route("/scan", post(scan))
        .route("/sync", post(sync_track17))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn healthz() -> Json<Health> {
    Json(Health { ok: true })
}

async fn list_shipments(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::list_shipments(&state.pool).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => err(e),
    }
}

async fn today(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::due_today(&state.pool, &crate::models::today_utc_date()).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => err(e),
    }
}

async fn get_shipment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match db::shipment_by_id(&state.pool, &id).await {
        Ok(Some(v)) => Json(v).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}

async fn scan(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.scan_gmail().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => err(e),
    }
}

async fn sync_track17(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.sync_track17().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => err(e),
    }
}

fn err(e: anyhow::Error) -> axum::response::Response {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
}
