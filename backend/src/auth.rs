use crate::error::ApiError;
use crate::AppState;
use axum::extract::State;
use axum::response::{IntoResponse, Redirect};
use openidconnect::core::{CoreClient, CoreProviderMetadata, CoreResponseType};
use openidconnect::{
    AuthenticationFlow, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IssuerUrl, Nonce, RedirectUrl, Scope,
};
use reqwest::ClientBuilder;
use sqlx::query;
use std::env;
use thiserror::Error;
use tracing::{error, info};

// Define the Client type with the required trait bounds
type Client = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

#[derive(Error, Debug)]
pub enum OidcError {
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("HTTP client error: {0}")]
    HttpClientError(String),
    #[error("Provider metadata discovery failed: {0}")]
    DiscoveryError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
}

// Enum to represent supported OIDC providers
#[derive(Debug, Clone)]
pub enum AuthProvider {
    Google,
    // Microsoft, // Add more providers here in the future
}

impl AuthProvider {
    // Returns the issuer URL for the provider
    fn issuer_url(&self) -> &'static str {
        match self {
            AuthProvider::Google => "https://accounts.google.com",
            // AuthProvider::Microsoft => "https://login.microsoftonline.com/common/v2.0",
        }
    }

    // Returns the environment variable names for client ID and secret
    fn env_vars(&self) -> (&'static str, &'static str) {
        match self {
            AuthProvider::Google => ("GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET"),
            // AuthProvider::Microsoft => ("MICROSOFT_CLIENT_ID", "MICROSOFT_CLIENT_SECRET"),
        }
    }
}

pub struct OidcConfig {
    client_id: ClientId,
    client_secret: ClientSecret,
    redirect_url: RedirectUrl,
    provider: AuthProvider,
}

impl OidcConfig {
    pub fn from_env(provider: AuthProvider) -> Result<Self, OidcError> {
        let (client_id_var, client_secret_var) = provider.env_vars();

        let client_id = ClientId::new(
            env::var(client_id_var)
                .map_err(|_| OidcError::MissingEnvVar(client_id_var.to_string()))?,
        );
        let client_secret = ClientSecret::new(
            env::var(client_secret_var)
                .map_err(|_| OidcError::MissingEnvVar(client_secret_var.to_string()))?,
        );
        let redirect_url = RedirectUrl::new(
            env::var("REDIRECT_URL")
                .map_err(|_| OidcError::MissingEnvVar("REDIRECT_URL".to_string()))?,
        )
        .map_err(|err| OidcError::InvalidUrl(err.to_string()))?;

        Ok(Self {
            client_id,
            client_secret,
            redirect_url,
            provider,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OidcProvider {
    pub client: Client,
}

impl OidcProvider {
    // Initialize the OIDC provider
    pub async fn new(config: OidcConfig) -> Result<Self, OidcError> {
        info!("Initializing OIDC provider for {:?}...", config.provider);

        let issuer_url = IssuerUrl::new(config.provider.issuer_url().to_string())
            .map_err(|err| OidcError::InvalidUrl(err.to_string()))?;

        let http_client = ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|err| OidcError::HttpClientError(err.to_string()))?;

        info!("Discovering provider metadata for {:?}...", config.provider);
        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &http_client)
            .await
            .map_err(|err| OidcError::DiscoveryError(err.to_string()))?;

        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            config.client_id,
            Some(config.client_secret),
        )
        .set_redirect_uri(config.redirect_url);

        info!(
            "OIDC provider for {:?} initialized successfully.",
            config.provider
        );
        Ok(Self { client })
    }

    // Generate the authorization URL to which we'll redirect the user
    pub async fn authorize_url(&self, state: &AppState) -> Result<String, OidcError> {
        let csrf_token = CsrfToken::new_random;
        let nonce = Nonce::new_random;
        let (authorize_url, csrf_state, nonce) = self
            .client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                csrf_token,
                nonce,
            )
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .url();

        // Save the CSRF state and nonce in the database
        let query = query(
        "INSERT INTO app_data.oauth_states (state, nonce, expires_at) VALUES ($1, $2, NOW() + INTERVAL '5 minutes')",
        )
         .bind(csrf_state.secret())
         .bind(nonce.secret());

        // Execute the query
        query.execute(&*state.pool).await.map_err(|e| {
            eprintln!("Failed to execute query: {}", e);
            OidcError::DatabaseError(e.to_string())
        })?;

        Ok(authorize_url.to_string())
    }
}

pub async fn authorize(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let oidc_provider = state.oidc_providers.get("google").unwrap();
    let authorize_url = oidc_provider.authorize_url(&state).await.map_err(|e| {
        error!("Failed to generate authorization URL: {:?}", e);
        ApiError::InternalServerError
    })?;
    Ok(Redirect::temporary(&authorize_url).into_response())
}
