use axum::extract::{FromRequestParts, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;

use crate::domain::Dek;
use crate::domain::UserId;
use crate::ports::UserRepoBd;

use super::state::AppState;

pub struct AuthUser {
    pub id: UserId,
    pub token: String,
    pub dek: Dek,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let token = header
            .strip_prefix("Bearer ")
            .filter(|t| !t.is_empty())
            .ok_or(StatusCode::UNAUTHORIZED)?
            .to_string();

        let session = state
            .sessions
            .lock()
            .expect("sessions")
            .get(&token)
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)?;

        Ok(AuthUser {
            id: session.user_id,
            token,
            dek: session.dek,
        })
    }
}

#[derive(Serialize)]
struct MeResponse {
    id: i64,
    name: String,
}

pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/me", get(me))
        .route("/logout", post(logout))
}

async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<MeResponse>, StatusCode> {
    let user = state
        .users
        .get(auth.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    Ok(Json(MeResponse {
        id: user.id.0,
        name: user.name,
    }))
}

async fn logout(State(state): State<AppState>, auth: AuthUser) -> StatusCode {
    state
        .sessions
        .lock()
        .expect("sessions")
        .remove(&auth.token);

    StatusCode::NO_CONTENT
}
