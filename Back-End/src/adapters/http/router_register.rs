use axum::{routing::get, Router};

use crate::adapters::db::postgres_users::PostgresUsers;

use super::health_route::health;
use super::router_list_users::{users_routes, AppState};

pub fn router(users: PostgresUsers) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(users_routes())
        .with_state(AppState { users })
}
