use crate::auth::Auth;
use crate::config::AppState;
use crate::error::ApiError;
use crate::graph::{GraphError, GraphInfo};
use axum::{
    extract::{Extension, State},
    Json,
};
use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;
use tracing::{error, info};
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
    Json(request): Json<CreateGraphRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    request.validate()?;
    // Anonymous users cannot be part of any organizations
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

    // Convert description which is Option<String> to Option<&str>
    let description = request.description.as_deref();

    // TODO: Handle different error types
    let graph_info = GraphInfo::new(&request.name, description).map_err(|e| match e {
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
