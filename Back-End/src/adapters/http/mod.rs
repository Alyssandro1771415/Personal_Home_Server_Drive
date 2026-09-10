//! Adapter HTTP (Axum). Entra request, chama use case, devolve JSON.
pub mod health_route;
pub mod list_users_route;
pub mod router_register;
pub mod login_route;

pub use router_register::router;
