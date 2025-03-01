use crate::auth::{AuthProvider, OauthSession, Session};
use crate::config::AppState;
use crate::error::ApiError;
use crate::user::{FederatedUser, User};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use oauth2::{AuthorizationCode, CsrfToken, TokenResponse};
use openidconnect::{LanguageTag, TokenResponse as OidcTokenResponse};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Serialize)]
struct AuthResponse {
    url: String,
}

// Endpoint to start the oidc authorization flow
pub async fn authorize(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    info!("Creating new session");
    // Get the OIDC provider and generate the authorization URL
    let oidc_provider = state.oidc_providers.get("google").unwrap();
    // Creates oauth session and returns the authorization URL
    let authorize_url = oidc_provider
        .generate_oidc_auth_url(&state)
        .await
        .map_err(|e| {
            error!("Failed to generate authorization URL: {:?}", e);
            ApiError::InternalServerError
        })?;

    let response = AuthResponse { url: authorize_url };

    // Response with a 200 and the authorization URL as json {"url": "https://..."}
    Ok((StatusCode::OK, Json(response)))
}

#[derive(Debug, Deserialize)]
pub struct AuthCallback {
    code: String,
    state: String,
}

// AuthCallback will be passed in as a JSON body
pub async fn callback(
    State(state): State<AppState>,
    params: Json<AuthCallback>,
) -> Result<impl IntoResponse, ApiError> {
    info!("Received OIDC callback");

    // Verify oauth session exists and is valid
    let oidc_state = CsrfToken::new(params.state.clone());
    let oauth_session = OauthSession::from_state(&state, oidc_state)
        .await
        .map_err(|e| {
            error!("Failed to fetch oauth session: {}", e);
            ApiError::Unauthorized
        })?;

    // Verfy the callback state/nonce match the oauth_session
    let callback_state = CsrfToken::new(params.state.clone());

    // Verify both session and nonce
    if oauth_session.state.secret() != callback_state.secret() {
        error!("Session or state mismatch");
        oauth_session.delete(&state).await.map_err(|e| {
            error!("Failed to delete oauth session: {}", e);
            ApiError::InternalServerError
        })?;
        return Err(ApiError::Unauthorized);
    }

    // Get the OIDC provider
    let oidc_provider = state.oidc_providers.get("google").ok_or_else(|| {
        error!("OIDC provider not found");
        ApiError::InternalServerError
    })?;

    let code = AuthorizationCode::new(params.code.clone());
    let token_res = oauth_session
        .convert_auth_code(&state, &oidc_provider, code.clone())
        .await
        .map_err(|e| {
            error!("Failed to convert auth code: {}", e);
            ApiError::InternalServerError
        })?;

    let id_token = token_res.id_token().unwrap().clone();
    // get the signing key
    let id_token_verifier = oidc_provider.client.id_token_verifier();
    let nonce_verifier = oauth_session.nonce;
    // Verify the ID token and retrieve the claims
    let claims = id_token
        .claims(&id_token_verifier, &nonce_verifier)
        .map_err(|e| {
            error!("Failed to verify ID token: {:?}", e);
            ApiError::Unauthorized
        })?;

    let sub = claims.subject().clone();

    // Check if FederatedUser already exists in DB
    let federated_user = FederatedUser::from_sub(&*state.pool, AuthProvider::Google, sub.clone())
        .await
        .map_err(|e| {
            error!("Failed to fetch federated user: {:?}", e);
            ApiError::InternalServerError
        })?;

    // if the user exists create a session and attach it to the user
    if let Some(federated_user) = federated_user {
        info!(
            "User exists, creating new session: sub: {:?}, provider: {:?}",
            federated_user.sub, federated_user.provider
        );
        //let expires_at = claims.issue_time() + token_res.expires_in().unwrap();
        let session = Session::create(
            &*state.pool,
            federated_user.user_id,
            federated_user.id,
            token_res.refresh_token(),
            claims.issue_time() + token_res.expires_in().unwrap(), // Expires at
        )
        .await
        .map_err(|e| {
            error!("Failed to create session: {}", e);
            ApiError::InternalServerError
        })?;

        // return the session id as json
        return Ok((StatusCode::OK, Json(session.id.to_string())).into_response());
    }

    let language_tag = LanguageTag::new("en".to_string());
    let locale = claims.locale().unwrap_or(&language_tag);
    let first_name = claims
        .given_name()
        .map(|n| n.get(Some(locale))) // this returns Option<Option<&EndUserGivenName>>
        .flatten() // this returns Option<&EndUserGivenName>
        .map(|n| n.to_string())
        .unwrap_or("".to_string());

    let last_name = claims
        .family_name()
        .map(|n| n.get(Some(locale))) // this returns Option<Option<&EndUserFamilyName>>
        .flatten() // this returns Option<&EndUserFamilyName>
        .map(|n| n.to_string())
        .unwrap_or("".to_string());

    // return an error if the email is not present
    let email = claims
        .email()
        .clone()
        .ok_or_else(|| {
            error!("Email not present in claims");
            ApiError::Unauthorized
        })?
        .to_string();

    // If the user does not exist, create a new user and federated user
    let mut transaction = state.pool.begin().await?;
    let user = User::new(email.clone(), first_name, last_name);
    user.persist(&mut transaction).await.map_err(|e| {
        error!("Failed to create user: {:?}", e);
        ApiError::InternalServerError
    })?;

    let picture_url = claims
        .picture()
        .map(|p| p.get(Some(locale)))
        .flatten()
        .map(|p| p.to_string());

    let federated_user =
        FederatedUser::new(user.id, AuthProvider::Google, sub, Some(email), picture_url);

    federated_user
        .persist(&mut transaction)
        .await
        .map_err(|e| {
            error!("Failed to create federated user: {:?}", e);
            ApiError::InternalServerError
        })?;

    transaction.commit().await.map_err(|e| {
        error!("Failed to commit user creation transaction: {:?}", e);
        ApiError::InternalServerError
    })?;

    // Create a new session
    // TODO: Verify nonce
    // use openidconnect::Nonce;
    let expires_at = claims.issue_time() + token_res.expires_in().unwrap();
    let session = Session::create(
        &*state.pool,
        federated_user.user_id,
        federated_user.id,
        token_res.refresh_token(),
        expires_at,
    )
    .await
    .map_err(|e| {
        error!("Failed to create session: {}", e);
        ApiError::InternalServerError
    })?;

    // return the session id as json
    Ok((StatusCode::OK, Json(session.id.to_string())).into_response())
}
