use crate::auth::Auth;
use crate::error::ApiError;
use crate::user::User;
use axum::extract::Extension;
use axum::Json;

use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::Serialize;
use tracing::{error, info};

#[derive(Serialize)]
pub struct Profile {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub registered_at: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
}

impl From<User> for Profile {
    fn from(user: User) -> Self {
        Self {
            first_name: user.first_name,
            last_name: user.last_name,
            email: user.email,
            registered_at: user.created_at,
            last_update: user.updated_at,
        }
    }
}

// Endpoint to start the oidc authorization flow
// Return Profile as Json
pub async fn profile(
    Extension(auth): Extension<Auth>,
) -> Result<(StatusCode, Json<Profile>), ApiError> {
    let user = auth.user.ok_or_else(|| {
        error!("Unauthorized access: no valid user found in middleware");
        ApiError::Unauthorized
    })?;

    info!("Fetched user for profile: {:?}", user.id);
    let profile = Profile::from(user);
    Ok((StatusCode::OK, Json(profile)))
}
