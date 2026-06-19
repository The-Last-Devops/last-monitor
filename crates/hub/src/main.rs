//! last-monitor hub: ingest endpoint, JSON API, and the server-rendered web UI.
//!
//! Environment:
//!   CONFIG_DATABASE_URL  Postgres URL for the config DB (users, namespaces, configs)
//!   DATA_DATABASE_URL    Postgres URL for the data DB (metrics, TimescaleDB)
//!   BIND_ADDR            listen address, default 0.0.0.0:8080
#![allow(clippy::type_complexity, clippy::items_after_test_module)]

mod alert;
mod api;
mod auth;
mod data_admin;
mod db;
mod ingest;
mod probe;
mod rbac;
mod ui;
mod web;

use std::net::SocketAddr;

use anyhow::{Context, Result};
use axum::{routing::get, Router};
use tower_http::trace::TraceLayer;

/// Shared application state handed to every handler.
#[derive(Clone)]
pub struct AppState {
    /// Pool for the config DB (plain Postgres).
    pub config: sqlx::PgPool,
    /// Pool for the data DB (Postgres + TimescaleDB).
    pub data: sqlx::PgPool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    let state = db::connect().await?;
    db::migrate(&state).await?;
    auth::bootstrap_admin(&state.config).await?;
    db::bootstrap_local_server(&state.config).await?;
    data_admin::setup(&state.data).await;

    // Background engines.
    probe::spawn(state.clone());
    alert::spawn(state.clone());

    use axum::routing::{delete, patch, post};
    let app = Router::new()
        // pages
        .route("/", get(ui::dashboard))
        .route("/login", get(ui::login_page))
        .route("/monitors", get(ui::monitors_page))
        .route("/manage", get(ui::manage_redirect))
        .route("/manage/namespaces", get(ui::manage_namespaces))
        .route("/manage/servers", get(ui::manage_servers))
        .route("/manage/monitors", get(ui::manage_monitors))
        .route("/manage/notifications", get(ui::manage_notifications))
        .route("/manage/members", get(ui::manage_members))
        .route("/manage/status", get(ui::manage_status))
        .route("/manage/users", get(ui::manage_users))
        .route("/manage/data", get(ui::manage_data))
        .route("/server/{id}", get(ui::server_detail))
        // public, unauthenticated status page
        .route("/status/{slug}", get(ui::public_status))
        // live HTML fragments
        .route("/ui/summary", get(ui::frag_summary))
        .route("/ui/servers", get(ui::frag_servers))
        .route("/ui/monitors", get(ui::frag_monitors))
        // embedded static assets
        .route("/static/app.css", get(ui::app_css))
        .route("/static/htmx.min.js", get(ui::htmx_js))
        .route("/static/uPlot.iife.min.js", get(ui::uplot_js))
        .route("/static/uPlot.min.css", get(ui::uplot_css))
        .route("/healthz", get(|| async { "ok" }))
        // agent ingest (token-authenticated, not session)
        .route("/api/ingest", post(ingest::ingest))
        // auth
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/me", get(auth::me))
        // admin user provisioning + data management
        .route("/api/users", post(api::create_user))
        .route("/api/admin/data", get(api::data_stats))
        .route("/api/admin/retention", post(api::set_retention))
        // management (session + RBAC)
        .route(
            "/api/namespaces",
            get(api::list_namespaces).post(api::create_namespace),
        )
        .route("/api/namespaces/{id}/members", post(api::add_member))
        .route(
            "/api/namespaces/{id}/tokens",
            get(api::list_tokens).post(api::create_token),
        )
        .route("/api/tokens/{id}", delete(api::delete_token))
        .route("/api/tokens/{id}/servers", get(api::token_servers))
        .route("/api/namespaces/{id}/monitors", post(api::create_monitor))
        .route(
            "/api/namespaces/{id}/channels",
            get(api::list_channels).post(api::create_channel),
        )
        .route(
            "/api/namespaces/{id}/alerts",
            get(api::list_alerts).post(api::create_alert),
        )
        .route(
            "/api/namespaces/{id}/status-pages",
            post(api::create_status_page),
        )
        // edit / delete resources
        .route(
            "/api/servers/{id}",
            patch(api::patch_server).delete(api::delete_server),
        )
        .route(
            "/api/monitors/{id}",
            patch(api::patch_monitor).delete(api::delete_monitor),
        )
        .route("/api/channels/{id}", delete(api::delete_channel))
        .route("/api/alerts/{id}", delete(api::delete_alert))
        .route("/api/status-pages/{id}", delete(api::delete_status_page))
        .route(
            "/api/namespaces/{id}/members/{user_id}",
            delete(api::delete_member),
        )
        // read views (scoped to caller)
        .route("/api/servers", get(web::list_servers))
        .route("/api/servers/{id}/metrics", get(web::server_metrics))
        .route("/api/servers/{id}/containers", get(web::server_containers))
        .route("/api/servers/{id}/temps", get(web::server_temps))
        .route("/api/servers/{id}/gpu", get(web::server_gpu))
        .route("/api/monitors", get(web::list_monitors))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".into())
        .parse()
        .context("invalid BIND_ADDR")?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "hub listening");
    axum::serve(listener, app).await?;
    Ok(())
}
