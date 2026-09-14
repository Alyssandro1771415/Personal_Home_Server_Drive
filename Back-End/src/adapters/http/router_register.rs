use axum::{routing::get, Router};

use crate::adapters::db::postgres_users::PostgresUsers;

use super::health_route::health;
use super::list_users_route::users_routes;
use super::login_route::login_routes;
use super::register_route::register_routes;
use super::state::AppState;

pub fn router(users: PostgresUsers) -> Router {
    let state = AppState {
        users,
        sessions: Default::default(),
    };

    Router::new()
        .route("/health", get(health))
        .merge(users_routes())
        .merge(login_routes())
        .merge(register_routes())
        .with_state(state)
}
