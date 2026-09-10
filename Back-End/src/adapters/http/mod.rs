//! Adapter HTTP (Axum). Entra request, chama use case, devolve JSON.
pub mod health_route;
pub mod router_list_users;
pub mod router_register;

pub use router_register::router;
