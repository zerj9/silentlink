//use crate::config::AppState;
//use crate::error::ApiError;
//use axum::extract::State;
//use axum::response::IntoResponse;
//use axum::Json;
//use reqwest::StatusCode;
//use serde::Serialize;
//use tracing::{error, info};
//
//#[derive(Serialize)]
//struct CreateOrgRequest {
//    name: String,
//    description: String,
//}
//
//// Endpoint to start the oidc authorization flow
//pub async fn authorize(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
//    info!("Creating new session");
//    // Get the OIDC provider and generate the authorization URL
//    let oidc_provider = state.oidc_providers.get("google").unwrap();
//    // Creates oauth session and returns the authorization URL
//    let authorize_url = oidc_provider
//        .generate_oidc_auth_url(&state)
//        .await
//        .map_err(|e| {
//            error!("Failed to generate authorization URL: {:?}", e);
//            ApiError::InternalServerError
//        })?;
//
//    let response = AuthResponse { url: authorize_url };
//
//    // Response with a 200 and the authorization URL as json {"url": "https://..."}
//    Ok((StatusCode::OK, Json(response)))
//}
use uuid::Uuid;

use strum_macros::{Display, EnumString};

#[derive(Debug, Display, EnumString)]
pub enum Role {
    Admin,
    Member,
}

pub struct OrgUser {
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
}
