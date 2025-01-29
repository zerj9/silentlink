mod ag;
mod config;
mod edge;
mod error;
mod graph;
mod label;
mod user;
mod utils;
mod vertex;

use crate::config::{AppState, Config};

use axum::{
    routing::{get, post},
    Router,
};
use dotenvy::dotenv;
use sqlx::{postgres::PgPoolOptions, Executor, PgPool};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{self, TraceLayer};
use tracing::{info, Level};

// TODO: handle already exists errors from database

#[tokio::main]
async fn main() {
    // Initialise tracing
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_level(true)
        .with_env_filter("info,sqlx=warn")
        .init();

    // Load .env file if it exists
    dotenv().ok();

    // Initialize configuration
    let config = Config::from_env().expect("Failed to load configuration from environment");

    // Create the connection pool with configuration
    let pool = Arc::new(
        PgPoolOptions::new()
            .max_connections(config.max_connections)
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    conn.execute("LOAD 'age'").await?;
                    conn.execute("SET search_path = ag_catalog, \"$user\", public")
                        .await?;
                    Ok(())
                })
            })
            .connect(&config.database_url)
            .await
            .expect("Failed to create pool"),
    );

    // run migrations
    sqlx::migrate!("./migrations")
        .run(pool.as_ref())
        .await
        .expect("Failed to run migrations");

    // Initialize AppState
    let state = AppState {
        pool: Arc::clone(&pool),
        graph_name: config.graph_name.clone(),
    };

    // Create router with all endpoints
    let app = Router::new()
        .route("/graphs", post(graph::create_graph))
        .route("/schema/nodes/labels", post(vertex::create_node_label))
        .route("/schema/edges/labels", post(edge::create_edge_label))
        .route("/nodes", post(vertex::create_node))
        .route("/nodes/:name", get(vertex::get_node_by_name))
        .with_state(state)
        .layer(TimeoutLayer::new(Duration::from_secs(10)))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        );

    let bind_address = env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3210".to_string());

    let listener = tokio::net::TcpListener::bind(bind_address).await.unwrap();
    info!(
        "axum: starting service on {}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app).await.unwrap();
}
