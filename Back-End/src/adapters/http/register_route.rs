use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

use crate::domain::Erro;
use crate::use_cases::register::Register;

use super::state::AppState;

#[derive(Deserialize)]
struct RegisterJson {
    nome: String,
    password: String,
}

#[derive(Serialize)]
struct RegisterResponse {
    id: i64,
}

pub fn register_routes() -> Router<AppState> {
    Router::new().route("/users", post(register))
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterJson>,
) -> Result<(StatusCode, Json<RegisterResponse>), StatusCode> {
    let uc = Register {
        repo: Arc::new(state.users.clone()),
        crypto: Arc::new(state.crypto),
        store: Arc::new(state.store.clone()),
    };

    let id = uc
        .execute(payload.nome, payload.password)
        .await
        .map_err(|err| match err {
            Erro::Validacao => StatusCode::BAD_REQUEST,
            Erro::Conflito => StatusCode::CONFLICT,
            Erro::Interno | Erro::NaoAutorizado | Erro::NaoEncontrado => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse { id: id.0 }),
    ))
}
