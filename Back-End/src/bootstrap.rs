use crate::adapters::db::memory::InMemoryUsers;
use crate::adapters::http;

pub async fn run() {
    let users = InMemoryUsers::with_seed();
    let app = http::router(users);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:5000")
        .await
        .expect("Falha ao iniciar o servidor na porta 5000");

    println!("Servidor inicializado na porta 5000");

    axum::serve(listener, app)
        .await
        .expect("Falha no servidor");
}
