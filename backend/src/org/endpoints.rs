use crate::auth::Auth;
use crate::config::AppState;
use crate::error::ApiError;
use crate::org::{Org, OrgMember};
use crate::user::User;

use axum::extract::{Extension, Path, State};
use axum::response::IntoResponse;
use axum::Json;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info};
use uuid::Uuid;

use super::Role;

#[derive(Debug, Deserialize)]
pub struct CreateOrgRequest {
    name: String,
    description: String,
}

#[axum::debug_handler]
pub async fn create_org(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Json(body): Json<CreateOrgRequest>,
) -> Result<StatusCode, ApiError> {
    // Anonymous users cannot create organizations
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

    info!("Creating new organization: {}", body.name);
    let org = Org::new(&body.name, &body.description);
    org.persist(&state.pool, user).await.map_err(|e| {
        error!("Failed to create organization: {:?}", e);
        ApiError::InternalServerError
    })?;

    Ok(StatusCode::CREATED)
}

#[derive(Serialize)]
pub struct OrgMemberSummaryResponse {
    id: String,
    name: String,
    description: String,
    role: Role,
}

pub async fn get_orgs(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
) -> Result<impl IntoResponse, ApiError> {
    // Anonymous users cannot be part of any organizations
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

    let org_memberships = user.get_org_memberships(&state.pool).await.map_err(|e| {
        error!("Failed to fetch org memberships: {:?}", e);
        ApiError::InternalServerError
    })?;
    let org_ids: Vec<_> = org_memberships.iter().map(|m| m.org_id).collect();

    let orgs = Org::get_many(&state.pool, org_ids).await.map_err(|e| {
        error!("Failed to fetch orgs: {:?}", e);
        ApiError::InternalServerError
    })?;

    let membership_map: HashMap<Uuid, &OrgMember> = org_memberships
        .iter()
        .map(|m| (m.org_id.clone(), m))
        .collect();

    let org_summaries: Vec<OrgMemberSummaryResponse> = orgs
        .clone()
        .into_iter()
        .filter_map(|org| {
            // Find the matching membership to get the role
            membership_map
                .get(&org.id)
                .map(|membership| OrgMemberSummaryResponse {
                    id: org.id.to_string(),
                    name: org.name,
                    description: org.description,
                    role: membership.role.clone(),
                })
        })
        .collect();

    Ok((StatusCode::OK, Json(org_summaries)))
}

#[derive(Debug, Deserialize)]
pub struct AddOrgMemberRequest {
    user_id: Uuid,
    role: Role,
}

pub async fn add_org_member(
    State(state): State<AppState>,
    Extension(auth): Extension<Auth>,
    Path(org_id): Path<Uuid>,
    Json(body): Json<AddOrgMemberRequest>,
) -> Result<StatusCode, ApiError> {
    let auth_user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

    let org = Org::from_id(&state.pool, org_id).await.map_err(|e| {
        error!("Failed to fetch org: {:?}", e);
        ApiError::InternalServerError
    })?;

    // Check that the reqesting member is an admin
    let requesting_member = org
        .get_member(&state.pool, auth_user.id)
        .await
        .map_err(|e| {
            error!("Failed to fetch org member: {:?}", e);
            ApiError::InternalServerError
        })?
        .ok_or_else(|| {
            error!("Requesting user is not a member of the org");
            ApiError::Unauthorized
        })?;

    if requesting_member.role != Role::Admin {
        error!("Requesting user is not an admin of the org");
        return Err(ApiError::Unauthorized);
    }

    // Check that the user to be added exists
    let user = User::from_id(&state.pool, body.user_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch user: {:?}", e);
            ApiError::InternalServerError
        })?;

    // Check that the user to be added is not already a member
    let existing_member = org.get_member(&state.pool, user.id).await.map_err(|e| {
        error!("Failed to fetch org member: {:?}", e);
        ApiError::InternalServerError
    })?;
    if existing_member.is_some() {
        error!("User is already a member of the org");
        return Err(ApiError::InternalServerError);
    }

    // Add the user to the org
    org.add_member(&state.pool, user, body.role)
        .await
        .map_err(|e| {
            error!("Failed to add user to org: {:?}", e);
            ApiError::InternalServerError
        })?;

    Ok(StatusCode::CREATED)
}
