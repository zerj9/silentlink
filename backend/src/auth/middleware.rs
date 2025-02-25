use crate::user::User;
use crate::{auth::Session, config::AppState};
use axum::{
    body::Body,
    extract::{FromRef, State},
    http::Request,
    middleware::Next,
    response::Response,
};
use axum_extra::headers::{authorization::Bearer, Authorization};
use axum_extra::TypedHeader;

#[derive(Debug, Clone, FromRef)]
pub struct Auth {
    pub user: Option<User>,
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    TypedHeader(Authorization(bearer)): TypedHeader<Authorization<Bearer>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let mut request = request;
    let token = bearer.token();

    if token.is_empty() {
        // Bearer token is not set. Handle accordingly.
        request.extensions_mut().insert(Auth { user: None });
        return next.run(request).await;
    }

    // Convert token to UUID
    let token = match uuid::Uuid::parse_str(&token) {
        Ok(token) => token,
        Err(_) => {
            // If token is invalid, pass through with no user.
            request.extensions_mut().insert(Auth { user: None });
            return next.run(request).await;
        }
    };

    // Attempt to get session, but do not block request if not found.
    let session = match Session::from_id(&state.pool, token).await {
        Ok(session) => Some(session),
        Err(e) => {
            tracing::warn!("Session not found or error: {}", e);
            None
        }
    };

    // Attempt to get user if session exists.
    let user = if let Some(session) = session {
        match User::from_id(&state.pool, session.user_id).await {
            Ok(user) => Some(user),
            Err(e) => {
                tracing::warn!("User not found or error: {}", e);
                None
            }
        }
    } else {
        None
    };

    request.extensions_mut().insert(Auth { user });
    next.run(request).await
}
