use axum::{routing::get, Router};

use crate::adapters::db::postgres_users::PostgresUsers;

use super::health_route::health;
use super::list_users_route::{users_routes, AppState};
use super::login_route::{login_routes, AppState as AppStateLogin};

pub fn router(users: PostgresUsers) -> Router {
    let login_router = login_routes().with_state(AppStateLogin {
        users: users.clone(),
    });

    Router::new()
        .route("/health", get(health))
        .merge(users_routes())
        .merge(login_router)
        .with_state(AppState { users })
}
