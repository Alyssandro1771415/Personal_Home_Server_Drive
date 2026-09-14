use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;

use crate::use_cases::list_users_bd::ListUsers;

use super::state::AppState;

#[derive(Serialize)]
struct UserJson {
    id: i64,
    name: String,
}

pub fn users_routes() -> Router<AppState> {
    Router::new().route("/users", get(list_users))
}

async fn list_users(
    State(state): State<AppState>,
) -> Result<Json<Vec<UserJson>>, StatusCode> {
    let uc = ListUsers {
        repo: Arc::new(state.users),
    };

    let users = uc
        .execute()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let body = users
        .into_iter()
        .map(|u| UserJson {
            id: u.id.0,
            name: u.name,
        })
        .collect();

    Ok(Json(body))
}
