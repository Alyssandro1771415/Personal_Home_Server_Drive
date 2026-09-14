use crate::adapters::db::postgres_users::PostgresUsers;
use crate::adapters::http;

use crate::adapters::db::postgres_connection;

use std::env;
use std::path::Path;

pub async fn run() {
    let env_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    dotenvy::from_path(&env_path).expect("não foi possível ler o .env em Back-End/");

    let pool = postgres_connection::postgres_connection()
        .await
        .expect("Falha ao conectar ao banco de dados");

    let users = PostgresUsers::new(pool);
    let app = http::router(users);

    let server_host: String = env::var("SERVER_HOST").unwrap_or_else(|_| "localhost".to_string());
    let server_port = env::var("SERVER_PORT").unwrap_or_else(|_| "5000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("{server_host}:{server_port}"))
        .await
        .expect("Falha ao iniciar o servidor na porta 5000");

    println!("Servidor inicializado na porta 5000");

    axum::serve(listener, app)
        .await
        .expect("Falha no servidor");
}
