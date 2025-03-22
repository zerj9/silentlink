mod ag;
mod auth;
mod config;
mod edge;
mod error;
mod graph;
//mod label;
mod node;
mod org;
mod user;
mod utils;

use crate::config::{AppState, Config};

use axum::{
    http::{HeaderValue, Method},
    middleware,
    routing::{get, post},
    Router,
};
use dotenvy::dotenv;
use maplit::hashmap;
use sqlx::{postgres::PgPoolOptions, Executor};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{self, TraceLayer};
use tracing::{info, Level};

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
    // Requires the AGE extension to be installed in the database
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

    // Initialize OIDC providers (Only Google for now)
    let google_oidc_config = auth::OidcConfig::from_env(auth::AuthProvider::Google)
        .expect("Failed to load OIDC configuration from environment");
    let google_oidc_provider = auth::OidcProvider::new(google_oidc_config).await.unwrap();

    // Initialize AppState
    let state = AppState {
        pool: Arc::clone(&pool),
        oidc_providers: hashmap! {
            "google".to_string() => google_oidc_provider,
        },
    };

    let cors = CorsLayer::new()
        .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
        .allow_methods(vec![Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);

    // Create router with all endpoints
    let app = Router::new()
        .route("/profile", get(user::profile))
        .route("/orgs", post(org::create_org))
        .route("/orgs", get(org::get_orgs))
        .route("/orgs/:id/members", post(org::add_org_member))
        .route("/orgs/:id/members", get(org::get_org_members))
        .route("/orgs/:id/graphs", post(graph::create_graph))
        .route("/orgs/:id/graphs", get(graph::get_graphs))
        .route("/graphs/:graph_id", get(graph::get_graph))
        // Node endpoints
        .route(
            "/graphs/:graph_id/meta/node_types",
            post(node::create_node_type),
        )
        .route(
            "/graphs/:graph_id/meta/node_types",
            get(node::get_node_types),
        )
        .route("/graphs/:graph_id/nodes", post(node::create_node))
        .route("/graphs/:graph_id/nodes", get(node::get_nodes))
        // Edge endpoints
        .route(
            "/graphs/:graph_id/meta/edge_types",
            post(edge::create_edge_type),
        )
        .route(
            "/graphs/:graph_id/meta/edge_types",
            get(edge::get_edge_types),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .route("/auth/url", post(auth::authorize))
        .route("/oidc/callback", post(auth::callback))
        .with_state(state)
        .layer(TimeoutLayer::new(Duration::from_secs(10)))
        .layer(cors)
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
