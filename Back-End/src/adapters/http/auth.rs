use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use super::state::AppState;


fn user_id_from_request(state: &AppState, headers: &HeadMap) -> Result<UserId, StatusCode> {
    let authorization: String = headers.get("Authorization");

    println!("{authorization}");
}
