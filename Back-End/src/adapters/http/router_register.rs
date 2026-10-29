use std::env;

use axum::extract::DefaultBodyLimit;
use axum::http::Method;
use axum::{routing::get, Router};
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer};

use super::auth::auth_routes;
use super::files_route::files_routes;
use super::folders_route::folders_routes;
use super::health_route::health;
use super::list_users_route::users_routes;
use super::login_route::login_routes;
use super::register_route::register_routes;
use super::state::AppState;
use super::trash_route::trash_routes;

fn max_upload_bytes() -> usize {
    let mb: usize = env::var("MAX_UPLOAD_MB")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|m: &usize| *m > 0)
        .unwrap_or(1024);
    mb.saturating_mul(1024 * 1024)
}

fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
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
        .merge(trash_routes())
        .layer(DefaultBodyLimit::max(max_upload_bytes()))
        .layer(cors())
        .with_state(state)
}
