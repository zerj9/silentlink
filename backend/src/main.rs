use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{postgres::PgPoolOptions, Error as SqlxError, PgPool, Row};
use std::collections::HashMap;
use std::env;
use std::num::ParseIntError;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{self, TraceLayer};
use tracing::{error, info, Level};
use validator::{Validate, ValidationError, ValidationErrors};

// TODO: handle already exists errors from database

// TODO: Implement permissions. Ignore for now.
//#[derive(Debug)]
//enum Permission {
//    CreateLabel,
//    DropLabel,
//    CreateNode,
//    DeleteNode,
//    // etc.
//}

#[derive(Debug)]
struct Config {
    database_url: String,
    max_connections: u32,
    graph_name: String,
}

#[derive(Debug, Error)]
enum ConfigError {
    #[error("Missing environment variable: {0}")]
    MissingVar(String),
    #[error("Invalid value for {0}: {1}")]
    InvalidValue(String, String),
}

impl Config {
    fn from_env() -> Result<Self, ConfigError> {
        // Load .env file if it exists
        dotenv().ok();

        let database_url = env::var("DATABASE_URL")
            .map_err(|_| ConfigError::MissingVar("DATABASE_URL".to_string()))?;

        let max_connections = env::var("PG_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "20".to_string())
            .parse::<u32>()
            .map_err(|e: ParseIntError| {
                ConfigError::InvalidValue("PG_MAX_CONNECTIONS".to_string(), e.to_string())
            })?;

        let graph_name = env::var("GRAPH_NAME")
            .map_err(|_| ConfigError::MissingVar("GRAPH_NAME".to_string()))?;

        Ok(Config {
            database_url,
            max_connections,
            graph_name,
        })
    }
}

#[derive(Clone)]
struct AppState {
    pool: Arc<PgPool>,
    graph_name: String,
}

// Create a struct to represent valid Cypher queries
#[derive(Debug)]
enum CypherQuery {
    CreateNode,
    CreateEdge,
    GetNodesByLabel,
    CreateNodeLabel,
    CreateEdgeLabel,
    CheckNodesExist,
}

impl CypherQuery {
    // Return the static query string for each variant
    fn as_str(&self) -> (&'static str, &'static str) {
        match self {
            CypherQuery::CreateNode => (
                "SELECT * FROM ag_catalog.cypher($1, $2, $3) as (id agtype)",
                "CREATE (n:$label) SET n = $props RETURN id(n) as id",
            ),
            CypherQuery::CreateEdge => (
                "SELECT * FROM ag_catalog.cypher($1, $2, $3) as (id agtype)",
                "MATCH (a), (b) 
                 WHERE id(a) = $from_id AND id(b) = $to_id
                 CREATE (a)-[r:$label]->(b)
                 SET r = $props
                 RETURN id(r) as id",
            ),
            CypherQuery::GetNodesByLabel => (
                "SELECT * FROM ag_catalog.cypher($1, $2, $3) as (id agtype, props agtype)",
                "MATCH (n:$label) RETURN id(n) as id, properties(n) as props",
            ),
            CypherQuery::CreateNodeLabel => (
                "SELECT ag_catalog.create_vlabel($1, $2)",
                "", // No Cypher query needed for catalog functions
            ),
            CypherQuery::CreateEdgeLabel => (
                "SELECT ag_catalog.create_elabel($1, $2)",
                "", // No Cypher query needed for catalog functions
            ),
            CypherQuery::CheckNodesExist => (
                "SELECT * FROM ag_catalog.cypher($1, $2, $3) as (count agtype)",
                "MATCH (n) WHERE id(n) IN [$node_id1, $node_id2] RETURN count(*) as count",
            ),
        }
    }
}

// Generic parameter map that can hold any named parameters
#[derive(Debug, Serialize, Default)]
struct CypherParams {
    params: HashMap<String, JsonValue>,
}

impl CypherParams {
    fn new() -> Self {
        Self {
            params: HashMap::new(),
        }
    }

    fn add<T: Serialize>(&mut self, name: &str, value: T) -> Result<(), serde_json::Error> {
        let value = serde_json::to_value(value)?;
        self.params.insert(name.to_string(), value);
        Ok(())
    }

    fn to_json(&self) -> Result<JsonValue, serde_json::Error> {
        serde_json::to_value(&self.params)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct GraphNode {
    id: Option<i64>,
    label: String,
    properties: HashMap<String, JsonValue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GraphEdge {
    id: Option<i64>,
    label: String,
    from_id: i64,
    to_id: i64,
    properties: HashMap<String, JsonValue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateLabelRequest {
    label: String,
}

impl CreateLabelRequest {
    // Validate label name according to common graph database naming rules
    fn validate(&self) -> Result<(), ApiError> {
        // Check if empty
        if self.label.is_empty() {
            return Err(ApiError::BadRequest("Label name cannot be empty".into()));
        }

        // Check length
        if self.label.len() > 50 {
            return Err(ApiError::BadRequest(
                "Label name too long (max 50 chars)".into(),
            ));
        }

        // Check if starts with letter
        if !self
            .label
            .chars()
            .next()
            .map_or(false, |c| c.is_ascii_alphabetic())
        {
            return Err(ApiError::BadRequest(
                "Label must start with a letter".into(),
            ));
        }

        // Check if contains only allowed characters (letters, numbers, and underscores)
        if !self
            .label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(ApiError::BadRequest(
                "Label can only contain letters, numbers, and underscores".into(),
            ));
        }

        Ok(())
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

// TODO: Implement error handling for API
#[derive(Debug, Error)]
enum ApiError {
    #[error("Database error: {0}")]
    Database(#[from] SqlxError),
    //#[error("Not found")]
    //NotFound,
    #[error("Invalid request: {0}")]
    BadRequest(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationErrors),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_response) = match self {
            ApiError::Database(ref e) => {
                // Handle specific database errors
                // Log the full error and code for debugging
                if let sqlx::Error::Database(db_err) = e {
                    error!(
                        "Database error occurred: {:?}, Code: {:?}",
                        e,
                        db_err.code()
                    );

                    // For AGE's "already exists" error
                    if db_err.message().contains("already exists") {
                        return (
                            axum::http::StatusCode::CONFLICT,
                            Json(ErrorResponse {
                                code: "LABEL_ALREADY_EXISTS".into(),
                                message: "The specified label already exists".into(),
                                details: None,
                            }),
                        )
                            .into_response();
                    }
                }
                // Default database error response
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorResponse {
                        code: "DATABASE_ERROR".into(),
                        message: "A database error occurred".into(),
                        details: None, // Don't expose internal error details
                    },
                )
            }
            ApiError::BadRequest(ref msg) => (
                axum::http::StatusCode::BAD_REQUEST,
                ErrorResponse {
                    code: "BAD_REQUEST".into(),
                    message: msg.clone(),
                    details: None,
                },
            ),
            ApiError::Serialization(ref e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                ErrorResponse {
                    code: "SERIALIZATION_ERROR".into(),
                    message: "Failed to process data".into(),
                    details: Some(e.to_string()),
                },
            ),
            ApiError::Validation(ref e) => (
                axum::http::StatusCode::BAD_REQUEST,
                ErrorResponse {
                    code: "VALIDATION_ERROR".into(),
                    message: "Invalid input data".into(),
                    details: Some(e.to_string()),
                },
            ),
        };

        (status, Json(error_response)).into_response()
    }
}

async fn create_node_label(
    State(state): State<AppState>,
    Json(payload): Json<CreateLabelRequest>,
) -> Result<Json<()>, ApiError> {
    // Validate the label name before proceeding
    payload.validate()?;

    let (age_query, _) = CypherQuery::CreateNodeLabel.as_str();
    sqlx::query(age_query)
        .bind(&state.graph_name)
        .bind(&payload.label)
        .execute(&*state.pool)
        .await?;

    Ok(Json(()))
}

async fn create_edge_label(
    State(state): State<AppState>,
    Json(payload): Json<CreateLabelRequest>,
) -> Result<Json<()>, ApiError> {
    // Validate the label name before proceeding
    payload.validate()?;

    let (age_query, _) = CypherQuery::CreateEdgeLabel.as_str();
    sqlx::query(age_query)
        .bind(&state.graph_name)
        .bind(&payload.label)
        .execute(&*state.pool)
        .await?;

    Ok(Json(()))
}

fn validate_label(label: &str) -> Result<(), ValidationError> {
    if !label
        .chars()
        .next()
        .map_or(false, |c| c.is_ascii_alphabetic())
    {
        return Err(ValidationError::new("label_must_start_with_letter"));
    }
    if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(ValidationError::new("invalid_label_characters"));
    }
    Ok(())
}

fn validate_properties(props: &HashMap<String, JsonValue>) -> Result<(), ValidationError> {
    // Check for maximum number of properties
    if props.len() > 100 {
        return Err(ValidationError::new("too_many_properties"));
    }

    // Validate property keys
    for key in props.keys() {
        if key.len() > 50 {
            return Err(ValidationError::new("property_key_too_long"));
        }
        if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(ValidationError::new("invalid_property_key_characters"));
        }
    }

    // Validate property values
    for value in props.values() {
        match value {
            JsonValue::String(s) if s.len() > 1000 => {
                return Err(ValidationError::new("string_value_too_long"));
            }
            JsonValue::Array(arr) => {
                if arr.len() > 100 {
                    return Err(ValidationError::new("array_too_large"));
                }
                // Validate array elements
                for elem in arr {
                    match elem {
                        JsonValue::String(s) if s.len() > 1000 => {
                            return Err(ValidationError::new("array_string_too_long"));
                        }
                        JsonValue::Array(_) => {
                            return Err(ValidationError::new("nested_arrays_not_allowed"));
                        }
                        JsonValue::Object(_) => {
                            return Err(ValidationError::new("objects_in_arrays_not_allowed"));
                        }
                        JsonValue::Null => {
                            return Err(ValidationError::new("null_values_not_allowed"));
                        }
                        JsonValue::Number(_) | JsonValue::Bool(_) | JsonValue::String(_) => {}
                    }
                }
            }
            JsonValue::Object(_) => {
                return Err(ValidationError::new("nested_objects_not_allowed"));
            }
            JsonValue::Null => {
                return Err(ValidationError::new("null_values_not_allowed"));
            }
            JsonValue::Number(_) | JsonValue::Bool(_) | JsonValue::String(_) => {}
        }
    }

    // Add numeric validation
    for value in props.values() {
        if let JsonValue::Number(n) = value {
            if let Some(n) = n.as_f64() {
                if !n.is_finite() || n.abs() > 1e308 {
                    return Err(ValidationError::new("numeric_value_out_of_bounds"));
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, Validate, Deserialize)]
struct CreateNodeRequest {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom = "validate_label")]
    label: String,
    #[validate(custom = "validate_properties")]
    properties: HashMap<String, JsonValue>,
}

async fn create_node(
    State(state): State<AppState>,
    Json(request): Json<CreateNodeRequest>,
) -> Result<Json<i64>, ApiError> {
    // Validate the request
    request.validate()?;

    let mut params = CypherParams::new();
    params.add("label", &request.label)?;
    params.add("props", &request.properties)?;

    let (age_query, cypher_query) = CypherQuery::CreateNode.as_str();
    let params_json = params.to_json()?;

    let row = sqlx::query(age_query)
        .bind(&state.graph_name)
        .bind(cypher_query)
        .bind(&params_json)
        .fetch_one(&*state.pool)
        .await?;

    let id = row.try_get::<i64, _>("id")?;
    Ok(Json(id))
}

#[derive(Debug, Validate, Deserialize)]
struct CreateEdgeRequest {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom = "validate_edge_type")]
    label: String,

    #[validate(range(min = 0))]
    from_id: i64,

    #[validate(range(min = 0))]
    to_id: i64,

    #[validate(custom = "validate_properties")]
    properties: HashMap<String, JsonValue>,
}

// TODO: Add additional validation functions specific to edges
// TODO: Check if edge label has been created, and if not, return an error
fn validate_edge_type(label: &str) -> Result<(), ValidationError> {
    // Add any edge-specific label validation rules
    if label.len() > 50 {
        return Err(ValidationError::new("edge_label_too_long"));
    }

    // Call the existing label validation
    validate_label(label)?;

    // Add edge-specific checks
    if label.contains("->") || label.contains("<-") {
        return Err(ValidationError::new("edge_label_contains_arrow"));
    }

    Ok(())
}

async fn validate_nodes_exist(
    pool: &PgPool,
    graph_name: &str,
    from_id: i64,
    to_id: i64,
) -> Result<(), ApiError> {
    // Create parameters using our flexible system
    let mut params = CypherParams::new();
    params.add("node_id1", &from_id)?;
    params.add("node_id2", &to_id)?;

    let (age_query, cypher_query) = CypherQuery::CheckNodesExist.as_str();
    let params_json = params.to_json()?;

    let row = sqlx::query(age_query)
        .bind(graph_name)
        .bind(cypher_query)
        .bind(&params_json)
        .fetch_one(pool)
        .await?;

    let count: i64 = row.try_get("count")?;
    if count != 2 {
        return Err(ApiError::BadRequest(
            "One or both nodes do not exist".into(),
        ));
    }
    Ok(())
}

async fn create_edge(
    State(state): State<AppState>,
    Json(request): Json<CreateEdgeRequest>,
) -> Result<Json<i64>, ApiError> {
    request.validate()?;

    let mut params = CypherParams::new();
    params.add("label", &request.label)?;
    params.add("props", &request.properties)?;
    params.add("from_id", &request.from_id)?;
    params.add("to_id", &request.to_id)?;

    // Check if nodes exist before creating the edge
    validate_nodes_exist(
        &*state.pool,
        &state.graph_name,
        request.from_id,
        request.to_id,
    )
    .await?;

    let (age_query, cypher_query) = CypherQuery::CreateEdge.as_str();
    let params_json = params.to_json()?;

    let row = sqlx::query(age_query)
        .bind(&state.graph_name)
        .bind(cypher_query)
        .bind(&params_json)
        .fetch_one(&*state.pool)
        .await?;

    let id = row.try_get::<i64, _>("id")?;
    Ok(Json(id))
}

async fn get_nodes_by_label(
    State(state): State<AppState>,
    Path(label): Path<String>,
) -> Result<Json<Vec<GraphNode>>, ApiError> {
    // Create parameters using our flexible system
    let mut params = CypherParams::new();
    params.add("label", &label)?;

    let (age_query, cypher_query) = CypherQuery::GetNodesByLabel.as_str();
    let params_json = params.to_json()?;

    let rows = sqlx::query(age_query)
        .bind(&state.graph_name)
        .bind(cypher_query)
        .bind(&params_json)
        .fetch_all(&*state.pool)
        .await?;

    let nodes = rows
        .iter()
        .map(|row| -> Result<GraphNode, ApiError> {
            let id = row.try_get("id").map_err(ApiError::Database)?;
            let props_value = row.try_get("props").map_err(ApiError::Database)?;
            let props = serde_json::from_value(props_value).map_err(ApiError::Serialization)?;

            Ok(GraphNode {
                id: Some(id),
                label: label.clone(),
                properties: props,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(nodes))
}

async fn initialize_graph(pool: &PgPool, graph_name: &str) -> Result<(), sqlx::Error> {
    let query = "SELECT ag_catalog.create_graph($1)";

    match sqlx::query(query).bind(graph_name).execute(pool).await {
        Ok(_) => {
            info!("Created new graph: {}", graph_name);
            Ok(())
        }
        Err(e) => {
            if let sqlx::Error::Database(ref db_error) = e {
                if db_error.code().as_deref() == Some("3F000")
                    && db_error.message().contains("already exists")
                {
                    info!("Graph {} already exists, continuing...", graph_name);
                    return Ok(());
                }
            }
            Err(e)
        }
    }
}

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
            .connect(&config.database_url)
            .await
            .expect("Failed to create pool"),
    );

    // Initialize AGE extension if not exists
    sqlx::query(
        "DO $$ 
        BEGIN 
            IF NOT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'age') THEN
                CREATE EXTENSION age;
            END IF;
        END $$;",
    )
    .execute(&*pool)
    .await
    .expect("Failed to initialize AGE extension");

    // Create graph if not exists
    initialize_graph(&*pool, &config.graph_name)
        .await
        .expect("Failed to initialize graph");

    // Initialize AppState
    let state = AppState {
        pool: Arc::clone(&pool),
        graph_name: config.graph_name.clone(),
    };

    // Create router with all endpoints
    let app = Router::new()
        .route("/schema/nodes/labels", post(create_node_label))
        .route("/schema/edges/labels", post(create_edge_label))
        .route("/nodes", post(create_node))
        .route("/nodes/label/:label", get(get_nodes_by_label))
        .route("/edges", post(create_edge))
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
