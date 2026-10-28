use axum::extract::DefaultBodyLimit;
use axum::{routing::get, Router};

use super::auth::auth_routes;
use super::files_route::files_routes;
use super::health_route::health;
use super::list_users_route::users_routes;
use super::login_route::login_routes;
use super::register_route::register_routes;
use super::state::AppState;

const MAX_UPLOAD_BYTES: usize = 32 * 1024 * 1024;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(users_routes())
        .merge(login_routes())
        .merge(register_routes())
        .merge(auth_routes())
        .merge(files_routes())
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
        .with_state(state)
}
