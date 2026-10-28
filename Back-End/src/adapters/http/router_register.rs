use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, Method};
use axum::{routing::get, Router};
use tower_http::cors::{AllowHeaders, CorsLayer};

use super::auth::auth_routes;
use super::files_route::files_routes;
use super::folders_route::folders_routes;
use super::health_route::health;
use super::list_users_route::users_routes;
use super::login_route::login_routes;
use super::register_route::register_routes;
use super::state::AppState;

const MAX_UPLOAD_BYTES: usize = 32 * 1024 * 1024;

fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://localhost:5173"),
            HeaderValue::from_static("http://127.0.0.1:5173"),
            HeaderValue::from_static("http://localhost:5174"),
            HeaderValue::from_static("http://127.0.0.1:5174"),
            HeaderValue::from_static("http://localhost:5175"),
            HeaderValue::from_static("http://127.0.0.1:5175"),
        ])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(AllowHeaders::mirror_request())
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(users_routes())
        .merge(login_routes())
        .merge(register_routes())
        .merge(auth_routes())
        .merge(files_routes())
        .merge(folders_routes())
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
        .layer(cors())
        .with_state(state)
}
