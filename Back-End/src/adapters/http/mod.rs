//! Adapter HTTP (Axum). Entra request, chama use case, devolve JSON.
pub mod health_route;
pub mod list_users_route;
pub mod login_route;
pub mod register_route;
pub mod router_register;
pub mod state;

pub use router_register::router;
