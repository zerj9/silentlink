use crate::ag::AgType;
use crate::config::AppState;
use crate::error::ApiError;
use crate::label::CreateLabelRequest;
use crate::utils::{generate_props_clause, validate_label, validate_properties};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_json::Value;
use sqlx::postgres::{PgRow, Postgres};
use sqlx::Transaction;
use sqlx::{FromRow, Row};
use std::collections::HashMap;
use tracing::info;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CreateNodeResponse {
    pub id: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Vertex {
    id: i64,
    label: String,
    properties: HashMap<String, JsonValue>,
}

// Implement FromRow for Vertex
// This uses the TryFrom implementation to convert AgType to Vertex
impl<'r> FromRow<'r, PgRow> for Vertex {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        // Extract the AgType from the row
        let ag_type: AgType = row.try_get("row")?; // Replace "ag_column" with the actual column name

        // Convert AgType to Vertex using TryFrom
        let vertex = Vertex::try_from(ag_type).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        Ok(vertex)
    }
}

impl Vertex {
    pub async fn get_by_name(
        state: &AppState,
        label: &str,
        name: &str,
    ) -> Result<Self, sqlx::Error> {
        let escaped_name = name.replace("'", "''");
        let query = format!(
            "SELECT * FROM cypher('{}', $$ MATCH (n:{} {{name: '{}'}}) RETURN n $$) as (row agtype)",
            &state.graph_name,
            &label,
            &escaped_name
        );

        sqlx::query_as::<_, Vertex>(&query)
            .fetch_one(&*state.pool)
            .await
    }
}

// Implement TryFrom for AgType to convert to Vertex
impl TryFrom<AgType> for Vertex {
    type Error = serde_json::Error;

    fn try_from(value: AgType) -> Result<Self, Self::Error> {
        serde_json::from_value(value.0)
    }
}

pub async fn create_node_label(
    State(state): State<AppState>,
    Json(payload): Json<CreateLabelRequest>,
) -> Result<Json<()>, ApiError> {
    // Validate the label name before proceeding
    payload.validate()?;
    // Uppercase the label name
    let label = payload.label.to_uppercase();

    // Start a transaction
    let mut transaction: Transaction<Postgres> = state.pool.begin().await?;

    let age_query = "SELECT ag_catalog.create_vlabel($1, $2)";
    sqlx::query(age_query)
        .bind(&state.graph_name)
        .bind(label)
        .execute(&mut *transaction)
        .await?;

    // Create hash map
    let mut vertex_properties = HashMap::new();
    vertex_properties.insert("name".to_string(), Value::String(payload.label.clone()));
    vertex_properties.insert(
        "display_name".to_string(),
        Value::String(payload.label.clone()),
    );

    let props_clause = generate_props_clause(&vertex_properties);

    let cypher_query = format!(
        "SELECT * FROM ag_catalog.cypher('{}', $$ CREATE (n:{} {}) RETURN n $$) as (row ag_catalog.agtype)",
        &state.graph_name,
        &payload.label,
        &props_clause
    );

    sqlx::query(&cypher_query)
        .execute(&mut *transaction)
        .await?;

    // Commit the transaction
    transaction.commit().await?;

    Ok(Json(()))
}

#[derive(Debug, Serialize, Deserialize)]
struct GraphNode {
    id: Option<i64>,
    label: String,
    properties: HashMap<String, JsonValue>,
}

#[derive(Debug, Validate, Deserialize)]
pub struct CreateNodeRequest {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom = "validate_label")]
    label: String,
    #[validate(custom = "validate_properties")]
    properties: HashMap<String, JsonValue>,
}

pub async fn create_node(
    State(state): State<AppState>,
    Json(request): Json<CreateNodeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    request.validate()?;

    // Fail if name is not provided
    if !request.properties.contains_key("name") {
        return Err(ApiError::BadRequest("Name property is required".into()));
    }

    // Do not allow creation of nodes with the same name
    let name = request.properties.get("name").unwrap().as_str().unwrap();
    let label = &request.label;

    // Check if a node with the same name already exists
    let existing_node = Vertex::get_by_name(&state, &label, name).await;
    info!("Existing node: {:?}", existing_node);
    if existing_node.is_ok() {
        return Err(ApiError::BadRequest(
            "Node with the same name already exists".into(),
        ));
    }

    let props_clause = generate_props_clause(&request.properties);

    let age_query = format!(
        "SELECT * FROM ag_catalog.cypher('{}', $$ CREATE (n:{} {}) RETURN n $$) as (row ag_catalog.agtype)",
        &state.graph_name,
        &request.label,
        &props_clause
    );

    // Execute the query and fetch the result as an AgTypeRow
    let vertex = sqlx::query_as::<_, Vertex>(&age_query)
        .fetch_one(&*state.pool)
        .await?;

    // Convert the Vertex to a JSON value and return it
    let vertex_value = serde_json::to_value(vertex)?;
    Ok(Json(vertex_value))
}

async fn get_node_label(
    State(state): State<AppState>,
    Path(label): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let query = format!(
        "SELECT * FROM cypher('{}', $$ MATCH (n:{}) RETURN n $$) as (row ag_catalog.agtype)",
        &state.graph_name, &label
    );

    let rows = sqlx::query_as::<_, AgType>(&query)
        .fetch_all(&*state.pool)
        .await?;

    let mut labels = Vec::new();
    for row in rows {
        let vertex: Vertex = serde_json::from_value(row.0)?;
        labels.push(vertex);
    }

    let labels_value = serde_json::to_value(labels)?;
    Ok(Json(labels_value))
}

pub async fn get_node_by_name(
    State(state): State<AppState>,
    Path((label, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let vertex = Vertex::get_by_name(&state, &label, &name).await?;
    let vertex_value = serde_json::to_value(vertex)?;
    Ok(Json(vertex_value))
}
