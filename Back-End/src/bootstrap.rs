use crate::adapters::crypto::AesGcmCrypto;
use crate::adapters::db::postgres_connection;
use crate::adapters::db::postgres_files::PostgresFiles;
use crate::adapters::db::postgres_folders::PostgresFolders;
use crate::adapters::db::postgres_users::PostgresUsers;
use crate::adapters::fs::FsStore;
use crate::adapters::http;
use crate::adapters::http::state::AppState;
use crate::use_cases::purge_expired::PurgeExpired;

use chrono::Duration;
use std::env;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration as StdDuration;

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

    let trash_days: i64 = env::var("TRASH_TTL_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|d: &i64| *d > 0)
        .unwrap_or(30);

    let files = PostgresFiles::new(pool.clone());
    let folders = PostgresFolders::new(pool.clone());
    let store = FsStore::new(storage_root);
    let trash_ttl = Duration::days(trash_days);

    let purge = PurgeExpired {
        files: Arc::new(files.clone()),
        folders: Arc::new(folders.clone()),
        store: Arc::new(store.clone()),
        ttl: trash_ttl,
    };
    tokio::spawn(async move {
        loop {
            let _ = purge.execute().await;
            tokio::time::sleep(StdDuration::from_secs(600)).await;
        }
    });

    let state = AppState {
        users: PostgresUsers::new(pool),
        files,
        folders,
        store,
        crypto: AesGcmCrypto,
        sessions: Default::default(),
        trash_ttl,
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
