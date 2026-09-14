use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::Deserialize;

use crate::adapters::db::postgres_users::PostgresUsers;
use crate::domain::UserId;
use crate::domain::Erro;
use crate::use_cases::login::GetHashVal;

#[derive(Clone)]
pub struct AppState {
    pub users: PostgresUsers,
}

#[derive(Deserialize)]
struct LoginJson {
    user_id: i64,
    password: String,
}

pub fn login_routes() -> Router<AppState> {
    Router::new().route("/login", post(get_result_val))
}

async fn get_result_val(
    State(state): State<AppState>,
    Json(payload): Json<LoginJson>,
) -> Result<StatusCode, StatusCode> {
    let uc = GetHashVal {
        repo: Arc::new(state.users),
    };

    uc.execute(UserId(payload.user_id), payload.password)
        .await
        .map_err(|err| match err {
            Erro::NaoAutorizado => StatusCode::UNAUTHORIZED,
            Erro::Interno => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    Ok(StatusCode::OK)
}
