use super::{EdgeTypeAttributeDefinition, NewEdgeTypeAttributeDefinition};
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
use serde::Deserialize;
use sqlx::{Postgres, Transaction};
use tracing::{error, info};

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
