use super::{
    EdgeTypeAttributeDataType, EdgeTypeAttributeDefinition, NewEdgeTypeAttributeDefinition,
};
use crate::auth::Auth;
use crate::config::AppState;
use crate::edge::EdgeType;
use crate::error::ApiError;
use crate::graph::GraphInfo;
use crate::org::{Org, Role};
use axum::{
    extract::{Extension, Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateEdgeTypeRequest {
    pub name: String,
    pub description: String,
    pub attributes: Vec<NewEdgeTypeAttributeDefinition>,
}

pub async fn create_edge_type(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path(graph_id): Path<String>,
    Json(payload): Json<CreateEdgeTypeRequest>,
) -> Result<Json<()>, ApiError> {
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

    // TODO: Add validation for the request payload
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
    // User is an admin of the org, proceed with creating the edge type
    //

    // TODO: Check if the edge type name is unique for the graph - case insensitive
    let edge_type =
        EdgeType::from_request(&payload, &graph_info.graph_id, user.id).map_err(|e| {
            error!("Failed to create edge type: {}", e);
            ApiError::BadRequest("Invalid edge type configuration".to_string())
        })?;

    let existing_edge_type =
        EdgeType::from_name(&state.pool, &graph_info.graph_id, &edge_type.name).await;
    if existing_edge_type.is_ok() {
        return Err(ApiError::BadRequest("Edge type already exists".to_string()));
    };

    // Start a transaction
    let mut transaction: Transaction<Postgres> = state.pool.begin().await.map_err(|e| {
        error!("Failed to start transaction for create_edge_label: {}", e);
        ApiError::InternalServerError
    })?;

    info!("Creating edge type for graph: {}", graph_info.name);
    edge_type.save(&mut transaction).await.map_err(|e| {
        error!("Failed to save edge type: {}", e);
        ApiError::InternalServerError
    })?;

    for new_attr in &payload.attributes {
        let attr = EdgeTypeAttributeDefinition::from_request(&new_attr, &edge_type.id);
        attr.save(&mut transaction).await.map_err(|e| {
            error!("Failed to save edge attribute: {}", e);
            ApiError::InternalServerError
        })?;
    }

    // Commit the transaction
    transaction.commit().await?;

    Ok(Json(()))
}

pub async fn get_edge_types(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path(graph_id): Path<String>,
) -> Result<Json<Vec<EdgeType>>, ApiError> {
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

    // Fetch all edge types for the graph
    let edge_types = EdgeType::list(&state.pool, &graph_id).await.map_err(|e| {
        error!("Failed to fetch edge types: {}", e);
        ApiError::InternalServerError
    })?;

    Ok(Json(edge_types))
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EdgeTypeAttributeResponse {
    pub id: Uuid,
    pub name: String,
    pub data_type: EdgeTypeAttributeDataType,
    pub required: bool,
    pub description: String,
}

impl EdgeTypeAttributeResponse {
    pub fn from(attr: &EdgeTypeAttributeDefinition) -> Self {
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
pub struct EdgeTypeResponse {
    pub id: String,
    pub graph_id: String,
    pub name: String,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: Uuid,
    pub attributes: Vec<EdgeTypeAttributeResponse>,
}

impl EdgeTypeResponse {
    pub fn from(node_type: &EdgeType, attributes: Vec<EdgeTypeAttributeDefinition>) -> Self {
        // Convert the attributes to the response type
        let attributes: Vec<EdgeTypeAttributeResponse> = attributes
            .iter()
            .map(|attr| EdgeTypeAttributeResponse::from(attr))
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

pub async fn get_edge_type(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path((graph_id, edge_type_id)): Path<(String, String)>,
) -> Result<Json<EdgeTypeResponse>, ApiError> {
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

    // Fetch the edge type
    let edge_type = EdgeType::from_id(&state.pool, &graph_info.graph_id, &edge_type_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch edge type: {}", e);
            ApiError::InternalServerError
        })?;

    let edge_type_attributes =
        EdgeTypeAttributeDefinition::from_edge_type(&state.pool, &edge_type.id)
            .await
            .map_err(|e| {
                error!("Failed to fetch edge type attributes: {}", e);
                ApiError::InternalServerError
            })?;

    let response = EdgeTypeResponse::from(&edge_type, edge_type_attributes);

    Ok(Json(response))
}
