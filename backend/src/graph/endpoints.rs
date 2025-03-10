use crate::auth::Auth;
use crate::config::AppState;
use crate::error::ApiError;
use crate::graph::{GraphError, GraphInfo};
use crate::org::{Org, Role};
use axum::{
    extract::{Extension, Path, State},
    Json,
};
use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;
use tracing::{error, info};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    // This regex matches only letters (both cases) and numbers.
    static ref NAME_REGEX: Regex = Regex::new(r"^[a-zA-Z0-9]+$").unwrap();
}

#[derive(Debug, Validate, Deserialize)]
pub struct CreateGraphRequest {
    #[validate(regex(
        path = "NAME_REGEX",
        message = "Name must contain only letters and numbers"
    ))]
    #[validate(length(max = 30, message = "Name must be at most 30 characters long"))]
    name: String,

    #[validate(length(max = 100, message = "Description must be at most 100 characters long"))]
    description: Option<String>,
}

pub async fn create_graph(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path(org_id): Path<Uuid>,
    Json(request): Json<CreateGraphRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    request.validate()?;
    // Anonymous users cannot be part of any organizations
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

    let org = Org::from_id(&state.pool, &org_id).await.map_err(|e| {
        error!("Failed to fetch organization: {:?}", e);
        ApiError::InternalServerError
    })?;

    // Check that the user is a member of the organization
    let org_member = org
        .get_member(&state.pool, user.id)
        .await
        .map_err(|e| {
            error!("Failed to fetch org member: {:?}", e);
            ApiError::InternalServerError
        })?
        .map_or_else(
            || {
                error!("User is not a member of the organization");
                Err(ApiError::Unauthorized)
            },
            |m| Ok(m),
        )?;

    // Check that the user is an admin of the organization
    if org_member.role != Role::Admin {
        error!("User is not an admin of the organization");
        return Err(ApiError::Unauthorized);
    }

    // Convert description which is Option<String> to Option<&str>
    let description = request.description.as_deref();

    // TODO: Handle different error types
    let graph_info = GraphInfo::new(&org, &request.name, description).map_err(|e| match e {
        GraphError::ValidationError(msg) => {
            error!("Validation error when creating graph: {}", msg);
            ApiError::BadRequest(msg)
        }
    })?;

    info!("Creating graph with name: {}", graph_info.name);
    graph_info.persist(&state.pool, user).await.map_err(|e| {
        info!("Failed to persist graph info: {:?}", e);
        ApiError::InternalServerError
    })?;

    Ok(Json(serde_json::json!({})))
}

pub async fn get_graphs(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path(org_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Anonymous users cannot be part of any organizations
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

    let org = Org::from_id(&state.pool, &org_id).await.map_err(|e| {
        error!("Failed to fetch organization: {:?}", e);
        ApiError::InternalServerError
    })?;

    // Check that the user is a member of the organization
    let org_member = org
        .get_member(&state.pool, user.id)
        .await
        .map_err(|e| {
            error!("Failed to fetch org member: {:?}", e);
            ApiError::InternalServerError
        })?
        .map_or_else(
            || {
                error!("User is not a member of the organization");
                Err(ApiError::Unauthorized)
            },
            |m| Ok(m),
        )?;

    if org_member.role != Role::Admin && org_member.role != Role::Viewer {
        error!("User is not an admin of the organization");
        return Err(ApiError::Unauthorized);
    }

    // Get all graphs for the organization
    let graphs = GraphInfo::get_all(&state.pool, org.id).await.map_err(|e| {
        error!("Failed to fetch graphs: {:?}", e);
        ApiError::InternalServerError
    })?;

    let response = graphs
        .iter()
        .map(|g| {
            serde_json::json!({
                "id": g.app_graphid,
                "name": g.name,
                "description": g.description.as_deref().unwrap_or(""),
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(serde_json::json!(response)))
}

pub async fn get_graph(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path(graph_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // TODO: Add functionality to allow public graphs to be viewed by anyone
    // Anonymous users cannot be part of any organizations
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

    // Get the graph by id
    let graph = GraphInfo::from_id(&state.pool, &graph_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch graph: {:?}", e);
            ApiError::InternalServerError
        })?;

    let org = Org::from_id(&state.pool, &graph.org_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch organization: {:?}", e);
            ApiError::InternalServerError
        })?;

    // Check that the user is a member of the organization
    let org_member = org
        .get_member(&state.pool, user.id)
        .await
        .map_err(|e| {
            error!("Failed to fetch org member: {:?}", e);
            ApiError::InternalServerError
        })?
        .map_or_else(
            || {
                error!("User is not a member of the organization");
                Err(ApiError::Unauthorized)
            },
            |m| Ok(m),
        )?;

    if org_member.role != Role::Admin && org_member.role != Role::Viewer {
        error!("User is not an admin of the organization");
        return Err(ApiError::Unauthorized);
    }

    let response = serde_json::json!({
        "id": graph.app_graphid,
        "name": graph.name,
        "description": graph.description.as_deref().unwrap_or(""),
    });

    Ok(Json(response))
}
