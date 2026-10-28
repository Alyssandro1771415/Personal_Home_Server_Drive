use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

use crate::domain::Erro;
use crate::use_cases::login::GetHashVal;

use super::state::{AppState, Session};

#[derive(Deserialize)]
struct LoginJson {
    nome: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    status_code: u16,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
}

pub fn login_routes() -> Router<AppState> {
    Router::new().route("/login", post(login))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginJson>,
) -> Result<(StatusCode, Json<LoginResponse>), (StatusCode, Json<LoginResponse>)> {
    let uc = GetHashVal {
        repo: Arc::new(state.users.clone()),
        crypto: Arc::new(state.crypto),
    };

    let (user_id, dek) = uc
        .execute(payload.nome, payload.password)
        .await
        .map_err(|err| match err {
            Erro::NaoAutorizado | Erro::Validacao | Erro::Conflito => (
                StatusCode::UNAUTHORIZED,
                Json(LoginResponse {
                    status_code: StatusCode::UNAUTHORIZED.as_u16(),
                    message: "Não autorizado.".to_string(),
                    token: None,
                }),
            ),
            Erro::Interno | Erro::NaoEncontrado => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(LoginResponse {
                    status_code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    message: "Erro interno do servidor.".to_string(),
                    token: None,
                }),
            ),
        })?;

    let token = uuid::Uuid::new_v4().to_string();
    state
        .sessions
        .lock()
        .expect("sessions")
        .insert(token.clone(), Session { user_id, dek });

    Ok((
        StatusCode::OK,
        Json(LoginResponse {
            status_code: StatusCode::OK.as_u16(),
            message: "Logado com sucesso!".to_string(),
            token: Some(token),
        }),
    ))
}
