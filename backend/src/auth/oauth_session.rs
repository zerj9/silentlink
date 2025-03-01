use crate::auth::OidcProvider;
use crate::config::AppState;
use oauth2::basic::BasicTokenType;
use openidconnect::{
    core::{CoreGenderClaim, CoreJweContentEncryptionAlgorithm, CoreJwsSigningAlgorithm},
    AuthorizationCode, CsrfToken, EmptyAdditionalClaims, EmptyExtraTokenFields, IdTokenFields,
    Nonce, PkceCodeVerifier, StandardTokenResponse,
};
use sqlx::{postgres::PgRow, FromRow, Row};
use thiserror::Error;
use tracing::error;

#[derive(Debug)]
pub struct OauthSession {
    pub state: CsrfToken,
    pub nonce: Nonce, // Nonce for OIDC verification
    pub pkce_verifier: PkceCodeVerifier,
}

// Implement FromRow for OauthSession to convert from PgRow to OauthSession
impl<'r> FromRow<'r, PgRow> for OauthSession {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let state: String = row.try_get("state")?;
        let state = CsrfToken::new(state);

        let nonce: String = row.try_get("nonce")?;
        let nonce = Nonce::new(nonce);

        let pkce_verifier: String = row.try_get("pkce_verifier")?;
        let pkce_verifier = PkceCodeVerifier::new(pkce_verifier);

        Ok(Self {
            state,
            nonce,
            pkce_verifier,
        })
    }
}

// Create OauthSessionError enum
#[derive(Debug, Error)]
pub enum OauthSessionError {
    #[error("Failed to create HTTP client: {0}")]
    NetworkError(String),
    //#[error("Failed to validate token: {0}")]
    //ValidationError(String),
}

impl OauthSession {
    pub fn new(state: CsrfToken, nonce: Nonce, pkce_verifier: PkceCodeVerifier) -> Self {
        Self {
            state,
            nonce,
            pkce_verifier,
        }
    }

    pub async fn persist(&self, state: &AppState) -> Result<(), sqlx::Error> {
        let query =
            "INSERT INTO app_data.oauth_session (state, nonce, pkce_verifier, expires_at) VALUES ($1, $2, $3, $4)";
        sqlx::query(query)
            .bind(self.state.secret())
            .bind(self.nonce.secret())
            .bind(self.pkce_verifier.secret())
            .bind(chrono::Utc::now() + chrono::Duration::days(730))
            .execute(&*state.pool)
            .await?;

        Ok(())
    }

    pub async fn from_state(app_state: &AppState, state: CsrfToken) -> Result<Self, sqlx::Error> {
        let query = "SELECT * FROM app_data.oauth_session WHERE state = $1 AND expires_at > NOW()";
        let row = sqlx::query_as::<_, OauthSession>(query)
            .bind(state.secret())
            .fetch_one(&*app_state.pool)
            .await?;

        Ok(row)
    }

    pub async fn delete(&self, state: &AppState) -> Result<(), sqlx::Error> {
        let query = "DELETE FROM app_data.oauth_session WHERE state = $1";
        sqlx::query(query)
            .bind(self.state.secret())
            .execute(&*state.pool)
            .await?;

        Ok(())
    }

    pub async fn convert_auth_code(
        &self,
        state: &AppState,
        provider: &OidcProvider,
        code: AuthorizationCode,
    ) -> Result<
        StandardTokenResponse<
            IdTokenFields<
                EmptyAdditionalClaims,
                EmptyExtraTokenFields,
                CoreGenderClaim,
                CoreJweContentEncryptionAlgorithm,
                CoreJwsSigningAlgorithm,
            >,
            BasicTokenType,
        >,
        OauthSessionError,
    > {
        // TODO:: Check if code can be validated

        // Create an HTTP client
        //let http_client = ClientBuilder::new()
        //    .redirect(reqwest::redirect::Policy::none())
        //    .timeout(std::time::Duration::from_secs(10))
        //    .build()
        //    .map_err(|err| OauthSessionError::NetworkError(err.to_string()))
        //    .map_err(|e| {
        //        error!("Failed to create HTTP client: {}", e);
        //        OauthSessionError::NetworkError(e.to_string())
        //    })?;

        // Exchange the authorization code for a token
        // Create a new PkceCodeVerifier as it doesn't implement Clone
        let verifier = PkceCodeVerifier::new(self.pkce_verifier.secret().clone());
        let token_response = provider
            .client
            .exchange_code(code)
            .map_err(|e| {
                error!("Failed to exchange code for token: {:?}", e);
                OauthSessionError::NetworkError(e.to_string())
            })?
            .set_pkce_verifier(verifier)
            .request_async(&provider.http_client)
            .await
            .map_err(|e| {
                error!("Failed to request token: {:?}", e);
                OauthSessionError::NetworkError(e.to_string())
            })?;

        // Delete oauth session
        self.delete(state).await.map_err(|e| {
            error!("Failed to delete oauth session: {}", e);
            OauthSessionError::NetworkError(e.to_string())
        })?;

        Ok(token_response)
    }
}
