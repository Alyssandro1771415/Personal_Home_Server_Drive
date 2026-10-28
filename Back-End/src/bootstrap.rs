use crate::adapters::crypto::AesGcmCrypto;
use crate::adapters::db::postgres_connection;
use crate::adapters::db::postgres_files::PostgresFiles;
use crate::adapters::db::postgres_users::PostgresUsers;
use crate::adapters::fs::FsStore;
use crate::adapters::http;
use crate::adapters::http::state::AppState;

use std::env;
use std::path::Path;

pub async fn run() {
    let env_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    dotenvy::from_path(&env_path).expect("não foi possível ler o .env em Back-End/");

    let pool = postgres_connection::postgres_connection()
        .await
        .expect("Falha ao conectar ao banco de dados");

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let storage_root = env::var("STORAGE_ROOT")
        .map(|p| {
            let path = Path::new(&p);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                manifest_dir.join(path)
            }
        })
        .unwrap_or_else(|_| manifest_dir.join("storage"));

    let state = AppState {
        users: PostgresUsers::new(pool.clone()),
        files: PostgresFiles::new(pool),
        store: FsStore::new(storage_root),
        crypto: AesGcmCrypto,
        sessions: Default::default(),
    };

    let app = http::router(state);

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
