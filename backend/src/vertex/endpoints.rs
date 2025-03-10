use crate::ag::AgType;
use crate::auth::Auth;
use crate::config::AppState;
use crate::error::ApiError;
use crate::graph::GraphInfo;
use crate::org::Org;
use crate::org::Role;
use crate::utils::{generate_props_clause, validate_label, validate_properties};
use crate::vertex::Vertex;
use axum::{
    extract::{Extension, Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use sqlx::{Postgres, Transaction};
use std::collections::HashMap;
use strum_macros::{AsRefStr, Display, EnumString};
use tracing::{error, info};
use validator::Validate;

#[derive(Debug, Deserialize)]
pub struct CreateNodeTypeRequest {
    pub label: String,
    pub description: String,
    pub attributes: Vec<AttributeDefinition>,
}

#[derive(Debug, Deserialize)]
pub struct AttributeDefinition {
    pub name: String,
    pub data_type: AttributeDataType,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Display, EnumString, AsRefStr)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AttributeDataType {
    String,
    Number,
    Boolean,
    Date,
    // Add other types as needed
}

pub async fn create_node_type(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path(graph_id): Path<String>,
    Json(payload): Json<CreateNodeTypeRequest>,
) -> Result<Json<()>, ApiError> {
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

    // TODO: Add validation for the request payload
    // Validate the label name before proceeding
    //payload.validate()?;

    let graph_info = GraphInfo::from_id(&state.pool, &graph_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch graph info: {}", e);
            ApiError::InternalServerError
        })?;

    let org = Org::from_id(&state.pool, &graph_info.org_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch organization: {}", e);
            ApiError::InternalServerError
        })?;

    // Check if the user is a member of the org
    let org_member = org
        .get_member(&state.pool, user.id)
        .await
        .map_err(|e| {
            error!("Failed to fetch org member: {}", e);
            ApiError::InternalServerError
        })?
        .ok_or_else(|| {
            error!("User is not a member of the organization");
            ApiError::Unauthorized
        })?;

    // Check if the user is an admin of the org
    if org_member.role != Role::Admin {
        return Err(ApiError::Unauthorized);
    }

    //
    // User is an admin of the org, proceed with creating the vertex label
    //

    // Uppercase the label name
    let type_name = payload.label.to_uppercase();

    // Start a transaction
    let mut transaction: Transaction<Postgres> = state.pool.begin().await.map_err(|e| {
        error!("Failed to start transaction for create_node_label: {}", e);
        ApiError::InternalServerError
    })?;

    // In AGE, node types are implemented as vertex labels
    let age_query = "SELECT ag_catalog.create_vlabel($1, $2)";
    sqlx::query(age_query)
        .bind(&graph_info.app_graphid)
        .bind(&type_name)
        .execute(&mut *transaction)
        .await
        .map_err(|e| {
            error!("Failed to execute CREATE node label query: {}", e);
            ApiError::InternalServerError
        })?;

    // Store node type metadata
    let insert_type = "
        INSERT INTO app_data.node_types (
            app_graphid, 
            type_name, 
            display_name,
            description, 
            created_by, 
            created_at
        ) VALUES ($1, $2, $3, $4, $5, NOW())";

    sqlx::query(insert_type)
        .bind(&graph_id)
        .bind(&type_name)
        .bind(&payload.label) // Original case for display_name
        .bind(&payload.description)
        .bind(user.id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| {
            error!("Failed to insert node type metadata: {}", e);
            ApiError::InternalServerError
        })?;

    // Store attributes for this node type
    for attr in &payload.attributes {
        let insert_attr = "
            INSERT INTO app_data.node_type_attributes (
                app_graphid,
                type_name,
                attribute_name,
                data_type,
                required,
                description
            ) VALUES ($1, $2, $3, $4, $5, $6)";

        sqlx::query(insert_attr)
            .bind(&graph_id)
            .bind(&type_name)
            .bind(&attr.name)
            .bind(&attr.data_type.to_string())
            .bind(attr.required)
            .bind(&attr.description)
            .execute(&mut *transaction)
            .await
            .map_err(|e| {
                error!("Failed to insert node type attribute: {}", e);
                ApiError::InternalServerError
            })?;
    }

    // Commit the transaction
    transaction.commit().await?;

    Ok(Json(()))
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
        .await
        .map_err(|e| {
            error!("Failed to execute CREATE node query: {}", e);
            ApiError::InternalServerError
        })?;

    // Convert the Vertex to a JSON value and return it
    let vertex_value = serde_json::to_value(vertex)?;
    Ok(Json(vertex_value))
}

async fn get_node_label(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path(label): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let query = format!(
        "SELECT * FROM cypher('{}', $$ MATCH (n:{}) RETURN n $$) as (row ag_catalog.agtype)",
        &state.graph_name, &label
    );

    let rows = sqlx::query_as::<_, AgType>(&query)
        .fetch_all(&*state.pool)
        .await
        .map_err(|e| {
            error!("Failed to execute MATCH node label query: {}", e);
            ApiError::InternalServerError
        })?;

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
