use crate::{
    db,
    models::{Health, ScanSummary, ShipmentRow, SyncSummary},
    service::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        healthz,
        list_shipments,
        today,
        get_shipment,
        scan,
        sync_track17,
    ),
    components(schemas(Health, ShipmentRow, ScanSummary, SyncSummary)),
    tags(
        (name = "health", description = "Health checks"),
        (name = "shipments", description = "Shipment queries"),
        (name = "operations", description = "Manual scan/sync triggers"),
    )
)]
struct ApiDoc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api-docs", get(scalar_ui))
        .route("/api-docs/openapi.json", get(openapi_json))
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

async fn scalar_ui() -> Html<&'static str> {
    Html(SCALAR_UI)
}

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[utoipa::path(
    get,
    path = "/healthz",
    tag = "health",
    responses(
        (status = 200, description = "Service health", body = Health)
    )
)]
async fn healthz() -> Json<Health> {
    Json(Health { ok: true })
}

#[utoipa::path(
    get,
    path = "/shipments",
    tag = "shipments",
    responses(
        (status = 200, description = "All known shipments", body = [ShipmentRow]),
        (status = 500, description = "Internal server error", body = String)
    )
)]
async fn list_shipments(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::list_shipments(&state.pool).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => err(e),
    }
}

#[utoipa::path(
    get,
    path = "/shipments/today",
    tag = "shipments",
    responses(
        (status = 200, description = "Shipments due today or out for delivery", body = [ShipmentRow]),
        (status = 500, description = "Internal server error", body = String)
    )
)]
async fn today(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::due_today(&state.pool, &crate::models::today_utc_date()).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => err(e),
    }
}

#[utoipa::path(
    get,
    path = "/shipments/{id}",
    tag = "shipments",
    params(
        ("id" = String, Path, description = "Shipment UUID")
    ),
    responses(
        (status = 200, description = "Shipment found", body = ShipmentRow),
        (status = 404, description = "Shipment not found"),
        (status = 500, description = "Internal server error", body = String)
    )
)]
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

#[utoipa::path(
    post,
    path = "/scan",
    tag = "operations",
    responses(
        (status = 200, description = "Gmail scan summary", body = ScanSummary),
        (status = 500, description = "Internal server error", body = String)
    )
)]
async fn scan(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.scan_gmail().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => err(e),
    }
}

#[utoipa::path(
    post,
    path = "/sync",
    tag = "operations",
    responses(
        (status = 200, description = "17TRACK sync summary", body = SyncSummary),
        (status = 500, description = "Internal server error", body = String)
    )
)]
async fn sync_track17(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.sync_track17().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => err(e),
    }
}

fn err(e: anyhow::Error) -> axum::response::Response {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
}

const SCALAR_UI: &str = r#"<!doctype html>
<html>
  <head>
    <title>Trackhound API Docs</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta
      http-equiv="Content-Security-Policy"
      content="default-src 'none'; script-src 'self' https://cdn.jsdelivr.net; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self';"
    />
  </head>
  <body>
    <script
      id="api-reference"
      data-url="/api-docs/openapi.json"
      data-theme="purple"
    ></script>
    <script
      src="https://cdn.jsdelivr.net/npm/@scalar/api-reference@1.57.2"
      integrity="sha384-v2ZO2RbUavIab3NxGfsSA1QP+HjzzXGRCKD5fTZu1xU4GFCjl1zjG1PNQ3V6/MpJ"
      crossorigin="anonymous"
    ></script>
  </body>
</html>
"#;
