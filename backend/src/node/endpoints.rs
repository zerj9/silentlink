use super::node_types;
use super::{
    Node, NodeType, NodeTypeAttributeDataType, NodeTypeAttributeDefinition, NodeTypeSummary,
};
use crate::auth::Auth;
use crate::config::AppState;
use crate::error::ApiError;
use crate::graph::GraphInfo;
use crate::node::{AttributeValidationError, CreateNodeError};
use crate::org::Org;
use crate::org::Role;
use axum::extract::Query;
//use crate::utils::{generate_props_clause, validate_label, validate_properties};
use axum::{
    extract::{Extension, Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::{Postgres, Transaction};
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize)]
pub struct NewAttributeDefinition {
    pub name: String,
    pub data_type: NodeTypeAttributeDataType,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateNodeTypeRequest {
    pub name: String,
    pub description: String,
    pub attributes: Vec<NewAttributeDefinition>,
}

pub async fn create_node_type(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path(graph_id): Path<String>,
    Json(payload): Json<CreateNodeTypeRequest>,
) -> Result<Json<JsonValue>, ApiError> {
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
    // User is an admin of the org, proceed with creating the node type
    //

    let node_type = node_types::NodeType::new(
        &graph_info.graph_id,
        &payload.name,
        payload.description,
        user.id,
    )
    .unwrap();

    // Check if the node type already exists
    let existing_node_type = NodeType::from_name(
        &state.pool,
        &graph_info.graph_id,
        &node_type.normalized_name,
    )
    .await;
    if existing_node_type.is_ok() {
        return Err(ApiError::BadRequest("Node type already exists".into()));
    }

    info!("Creating node type for graph: {}", graph_info.name);

    // Start a transaction
    let mut transaction: Transaction<Postgres> = state.pool.begin().await.map_err(|e| {
        error!("Failed to start transaction for create_node_label: {}", e);
        ApiError::InternalServerError
    })?;

    node_type.save(&mut transaction).await.map_err(|e| {
        error!("Failed to save node type: {}", e);
        ApiError::InternalServerError
    })?;

    // Store attributes for this node type
    for new_attr_def in &payload.attributes {
        let attr_def = NodeTypeAttributeDefinition::from_request(new_attr_def, &node_type.id);

        attr_def.save(&mut transaction).await.map_err(|e| {
            error!("Failed to save attribute: {}", e);
            ApiError::InternalServerError
        })?;
    }

    // Commit the transaction
    transaction.commit().await?;

    // Return Node Type ID
    Ok(Json(json!({"id": node_type.id})))
}

pub async fn get_node_types(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path(graph_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // TODO: Add functionality to allow public graphs to be viewed by anyone
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

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

    // Check if the user is an admin or viewer of the org
    if org_member.role != Role::Admin && org_member.role != Role::Viewer {
        return Err(ApiError::Unauthorized);
    }

    let node_types = graph_info.get_node_types(&state.pool).await.map_err(|e| {
        error!("Failed to fetch node types: {}", e);
        ApiError::InternalServerError
    })?;

    // Summarize the node types, From<NodeType> for NodeTypeSummary exists
    let node_type_summaries: Vec<NodeTypeSummary> = node_types
        .iter()
        .map(|node_type| NodeTypeSummary::from(node_type))
        .collect();

    Ok(Json(serde_json::json!(node_type_summaries)))
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NodeTypeAttributeResponse {
    pub id: Uuid,
    pub name: String,
    pub data_type: NodeTypeAttributeDataType,
    pub required: bool,
    pub description: String,
}

impl NodeTypeAttributeResponse {
    pub fn from(attr: &NodeTypeAttributeDefinition) -> Self {
        Self {
            id: attr.id.clone(),
            name: attr.name.clone(),
            data_type: attr.data_type.clone(),
            required: attr.required,
            description: attr.description.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NodeTypeResponse {
    pub id: String,
    pub graph_id: String,
    pub name: String,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: Uuid,
    pub attributes: Vec<NodeTypeAttributeResponse>,
}

impl NodeTypeResponse {
    pub fn from(node_type: &NodeType, attributes: Vec<NodeTypeAttributeDefinition>) -> Self {
        // Convert the attributes to the response type
        let attributes: Vec<NodeTypeAttributeResponse> = attributes
            .iter()
            .map(|attr| NodeTypeAttributeResponse::from(attr))
            .collect();
        Self {
            id: node_type.id.clone(),
            graph_id: node_type.graph_id.clone(),
            name: node_type.name.clone(),
            description: node_type.description.clone(),
            created_at: node_type.created_at,
            created_by: node_type.created_by,
            attributes,
        }
    }
}

pub async fn get_node_type(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path((graph_id, node_type_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // TODO: Add functionality to allow public graphs to be viewed by anyone
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

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

    // Check if the user is an admin or viewer of the org
    if org_member.role != Role::Admin && org_member.role != Role::Viewer {
        return Err(ApiError::Unauthorized);
    }

    let node_type = NodeType::from_id(&state.pool, &graph_info.graph_id, &node_type_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch node type: {}", e);
            ApiError::InternalServerError
        })?;

    let node_type_attributes = NodeTypeAttributeDefinition::from_node_type(&state.pool, &node_type)
        .await
        .map_err(|e| {
            error!("Failed to fetch node type attributes: {}", e);
            ApiError::InternalServerError
        })?;

    let response = NodeTypeResponse::from(&node_type, node_type_attributes);

    Ok(Json(serde_json::json!(response)))
}

#[derive(Debug, Validate, Deserialize)]
pub struct CreateNodeRequest {
    pub node_type: String,
    pub properties: HashMap<String, JsonValue>,
}

#[derive(Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

use validator::{ValidationError, ValidationErrors};
pub async fn create_node(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path(graph_id): Path<String>,
    Json(request): Json<CreateNodeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // TODO: Remove this, use a custom validation function
    request.validate()?;
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

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

    // Check if the node type exists
    NodeType::from_id(&state.pool, &graph_info.graph_id, &request.node_type)
        .await
        .map_err(|e| {
            error!("Failed to fetch node type: {}", e);
            ApiError::BadRequest("Node type does not exist".into())
        })?;

    // Fail if name is not provided
    if !request.properties.contains_key("name") {
        return Err(ApiError::BadRequest("Name property is required".into()));
    }

    // Do not allow creation of nodes with the same name
    let name = request.properties.get("name").unwrap().as_str().unwrap();
    let node_type = &request.node_type;

    // Check if a node of the same type with the same name already exists
    let existing_node =
        Node::get_by_name(&state.pool, &graph_info.graph_id, &node_type, name).await;
    if existing_node.is_ok() {
        warn!("Existing node: {:?}", existing_node);
        return Err(ApiError::BadRequest(
            "Node with the same name already exists".into(),
        ));
    }

    Node::create(&state.pool, request, user.id, graph_info.graph_id)
        .await
        .map_err(|e| match e {
            CreateNodeError::ValidationError(errors) => {
                let mut validation_errors = ValidationErrors::new();
                for error in errors {
                    match error {
                        AttributeValidationError::MissingAttribute { name } => {
                            let mut val_error = ValidationError::new("missing");
                            val_error.message = Some("required".into());
                            // Convert the dynamic field name into a &'static str.
                            validation_errors.add(Box::leak(name.into_boxed_str()), val_error);
                        }
                        AttributeValidationError::WrongType { name, expected } => {
                            let mut val_error = ValidationError::new("wrong_type");
                            val_error.message =
                                Some(format!("must be of type {}", expected).into());
                            validation_errors.add(Box::leak(name.into_boxed_str()), val_error);
                        }
                    }
                }
                ApiError::Validation(validation_errors)
            }
            CreateNodeError::DatabaseError(_) => {
                error!("Database error when creating node: {}", e);
                ApiError::InternalServerError
            }
        })?;

    Ok(Json(json!({})))
}

#[derive(Deserialize)]
pub struct GetNodesQueryParams {
    pub page: Option<u32>,
    pub node_type: Option<String>,
}

pub async fn get_nodes(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path(graph_id): Path<String>,
    Query(params): Query<GetNodesQueryParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // TODO: Allow public graphs to be viewed by anyone
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

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

    // Check if the user is an admin or viewer of the org
    if org_member.role != Role::Admin && org_member.role != Role::Viewer {
        return Err(ApiError::Unauthorized);
    }

    let nodes = Node::list(
        &state.pool,
        &graph_info.graph_id,
        params.node_type.as_deref(),
        params.page,
    )
    .await
    .map_err(|e| {
        error!("Failed to fetch nodes: {}", e);
        ApiError::InternalServerError
    })?;

    Ok(Json(serde_json::json!(nodes)))
}
