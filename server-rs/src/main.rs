pub mod config;
pub mod controllers;
pub mod dtos;
pub mod error;
pub mod crypto;
pub mod middleware;
pub mod models;
pub mod jobs;
pub mod ml;

use axum::{extract::DefaultBodyLimit, Router};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::trace::TraceLayer;
use tower_http::services::{ServeDir, ServeFile};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use jobs::{JobQueue, run_worker};

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub job_queue: Arc<JobQueue>,
    pub media_location: String,
    pub web_root: String,
    pub socket_tx: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server_rs=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = config::load();

    // The Immich DB connection defaults to postgres://postgres:postgres@localhost:5432/immich
    let db_url = if cfg.db_url.is_empty() {
        "postgres://postgres:postgres@localhost:5432/immich".to_string()
    } else {
        cfg.db_url
    };

    tracing::debug!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;
    run_migrations(&pool).await?;
    ensure_version_history(&pool).await?;
        
    let (job_queue, receiver) = JobQueue::new();
    let job_queue = Arc::new(job_queue);
    let receiver = Arc::new(tokio::sync::Mutex::new(receiver));

    let (socket_tx, _) = broadcast::channel(1024);

    let state = AppState {
        db: pool,
        job_queue: job_queue.clone(),
        media_location: cfg.media_location,
        web_root: cfg.web_root.clone(),
        socket_tx,
    };

    let worker_count = cfg.job_worker_concurrency.max(1);
    for worker_id in 0..worker_count {
        tokio::spawn({
            let worker_state = state.clone();
            let receiver = receiver.clone();
            async move {
                run_worker(receiver, worker_state, worker_id + 1).await;
            }
        });
    }

    let web_root = PathBuf::from(cfg.web_root);
    let index_html = web_root.join("index.html");

    let app = Router::new()
        .merge(controllers::app::router())
        .nest("/api/server", controllers::server::router())
        .nest("/api/users", controllers::user::router())
        .nest("/api/auth", controllers::auth::router())
        .nest("/api/albums", controllers::album::router())
        .nest("/api/assets", controllers::asset::router())
        .nest("/api/system-metadata", controllers::system_metadata::router())
        .nest("/api/memories", controllers::memory::router())
        .nest("/api/notifications", controllers::notification::router())
        .nest("/api/timeline", controllers::timeline::router())
        .nest("/api/sessions", controllers::session::router())
        .nest("/api/shared-links", controllers::shared_link::router())
        .nest("/api/stacks", controllers::stack::router())
        .nest("/api/search", controllers::search::router())
        .nest("/api/libraries", controllers::library::router())
        .nest("/api/download", controllers::download::router())
        .nest("/api/trash", controllers::trash::router())
        .nest("/api/map", controllers::map::router())
        .nest("/api/partners", controllers::partner::router())
        .nest("/api/tags", controllers::tag::router())
        .nest("/api/views", controllers::view::router())
        .nest("/api/admin/users", controllers::user_admin::router())
        .nest("/api/admin/auth", controllers::auth_admin::router())
        .nest("/api/admin/notifications", controllers::notification_admin::router())
        .nest("/api/system-config", controllers::system_config::router())
        .nest("/api/admin/maintenance", controllers::maintenance::router())
        .nest("/api/queues", controllers::queue::router())
        .nest("/api/jobs", controllers::job::router())
        .nest("/api/admin/database-backups", controllers::database_backup::router())
        .nest("/api/duplicates", controllers::duplicate::router())
        .nest("/api/api-keys", controllers::api_key::router())
        .nest("/api/activities", controllers::activity::router())
        .nest("/api/faces", controllers::face::router())
        .nest("/api/people", controllers::person::router())
        .nest("/api/sync", controllers::sync::router())
        .nest("/api/workflows", controllers::workflow::router())
        .nest("/api/plugins", controllers::plugin::router())
        .nest("/api/oauth", controllers::oauth::router())
        .merge(controllers::socket::router())
        .layer(DefaultBodyLimit::disable())
        .with_state(state.clone())
        .layer(TraceLayer::new_for_http())
        .fallback_service(
            ServeDir::new(&web_root).not_found_service(ServeFile::new(index_html)),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    tracing::debug!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn run_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let has_version_history = sqlx::query_scalar::<_, Option<String>>(
        "SELECT to_regclass('public.version_history')::text",
    )
    .fetch_one(pool)
    .await?;

    if has_version_history.is_none() {
        tracing::warn!("version_history missing; applying baseline schema SQL");
        sqlx::raw_sql(include_str!("../migrations/0001_baseline.sql"))
            .execute(pool)
            .await?;
        return Ok(());
    }

    // With the current Rust migration layout we only ship the full baseline schema.
    // Avoid re-running the same baseline through sqlx migrations on an initialized
    // database, which can fail on pre-existing extension schemas such as `vectors`.
    Ok(())
}

async fn ensure_version_history(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let version = current_immich_version();
    let latest = sqlx::query_scalar::<_, String>(
        r#"
        SELECT version
        FROM version_history
        ORDER BY "createdAt" DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    if latest.as_deref() != Some(version.as_str()) {
        sqlx::query(
            r#"
            INSERT INTO version_history (version)
            VALUES ($1)
            "#,
        )
        .bind(version)
        .execute(pool)
        .await?;
    }

    Ok(())
}

fn current_immich_version() -> String {
    serde_json::from_str::<serde_json::Value>(include_str!("../../server/package.json"))
        .ok()
        .and_then(|package| package.get("version").and_then(|value| value.as_str()).map(str::to_owned))
        .unwrap_or_else(|| "2.7.4".to_string())
}
