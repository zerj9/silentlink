use super::EdgeAttributeDefinition;
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
    pub attributes: Vec<EdgeAttributeDefinition>,
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
    // Uppercase the label name
    let type_name = payload.name.to_uppercase();
    let edge_type = EdgeType::new(&type_name, &payload.name, &payload.description, &user.id);

    // Start a transaction
    let mut transaction: Transaction<Postgres> = state.pool.begin().await.map_err(|e| {
        error!("Failed to start transaction for create_edge_label: {}", e);
        ApiError::InternalServerError
    })?;

    info!("Creating edge type for graph: {}", graph_info.name);
    let age_query = "SELECT ag_catalog.create_elabel($1, $2)";
    sqlx::query(age_query)
        .bind(&graph_info.graph_id)
        .bind(&type_name)
        .execute(&mut *transaction)
        .await
        .map_err(|e| {
            error!("Failed to execute CREATE edge label query: {}", e);
            ApiError::InternalServerError
        })?;

    info!(
        "Storing edge type metadata in database for graph: {}",
        graph_info.name
    );
    // Store edge type metadata
    let insert_type = "
        INSERT INTO app_data.edge_types (
            graph_id, 
            type_name, 
            display_name,
            description, 
            created_by, 
            created_at
        ) VALUES ($1, $2, $3, $4, $5, NOW())";

    sqlx::query(insert_type)
        .bind(&graph_id)
        .bind(&type_name)
        .bind(&payload.name) // Original case for display_name
        .bind(&payload.description)
        .bind(user.id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| {
            error!("Failed to insert edge type metadata: {}", e);
            ApiError::InternalServerError
        })?;

    // Store attributes for this edge type
    for attr in &payload.attributes {
        let insert_attr = "
            INSERT INTO app_data.edge_type_attributes (
                graph_id,
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
                error!("Failed to insert edge type attribute: {}", e);
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
    let edge_types = EdgeType::get_all(&state.pool, &graph_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch edge types: {}", e);
            ApiError::InternalServerError
        })?;

    Ok(Json(edge_types))
}
