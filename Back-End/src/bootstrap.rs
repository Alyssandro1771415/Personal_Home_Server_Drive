use crate::adapters::db::postgres_connection;
use crate::adapters::db::postgres_files::PostgresFiles;
use crate::adapters::db::postgres_folders::PostgresFolders;
use crate::adapters::db::postgres_users::PostgresUsers;
use crate::adapters::fs::FsStore;
use crate::adapters::http;
use crate::adapters::http::state::AppState;
use crate::domain::Erro;
use crate::use_cases::purge_expired::PurgeExpired;
use crate::use_cases::reset_password::ResetPassword;
use crate::use_cases::sync_disk::sync_all_users;

use chrono::Duration;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration as StdDuration;

fn load_env() {
    let _ = dotenvy::dotenv();
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    let _ = dotenvy::from_path(fallback);
}

fn resolve_storage_root() -> PathBuf {
    match env::var("STORAGE_ROOT") {
        Ok(p) if !p.trim().is_empty() => {
            let path = PathBuf::from(p.trim());
            if path.is_absolute() {
                path
            } else {
                env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(path)
            }
        }
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("storage"),
    }
}

pub async fn run() {
    let args: Vec<String> = env::args().skip(1).collect();
    load_env();

    let storage_root = resolve_storage_root();
    tokio::fs::create_dir_all(&storage_root)
        .await
        .expect("Falha ao criar diretório STORAGE_ROOT");

    let pool = postgres_connection::postgres_connection()
        .await
        .expect("Falha ao conectar ao banco de dados");

    postgres_connection::ensure_schema(&pool)
        .await
        .expect("Falha ao garantir o schema do banco de dados");

    let users = PostgresUsers::new(pool.clone());
    let files = PostgresFiles::new(pool.clone());
    let folders = PostgresFolders::new(pool);

    match args.first().map(String::as_str) {
        Some("reset-password") => {
            if args.len() != 3 {
                eprintln!("Uso: drive reset-password <usuario> <nova-senha>");
                std::process::exit(2);
            }
            let uc = ResetPassword {
                repo: Arc::new(users),
            };
            match uc.execute(&args[1], &args[2]).await {
                Ok(user) => {
                    println!(
                        "Senha redefinida com sucesso para o usuário '{}' (id {}).",
                        user.name, user.id.0
                    );
                }
                Err(Erro::Validacao) => {
                    eprintln!("Erro: informe um nome válido e senha com no mínimo 8 caracteres.");
                    std::process::exit(1);
                }
                Err(Erro::NaoEncontrado) => {
                    eprintln!("Erro: usuário '{}' não encontrado.", args[1]);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Erro interno ao redefinir senha: {e:?}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Some("sync-disk") => {
            match sync_all_users(&storage_root, &users, &folders, &files).await {
                Ok(stats) => {
                    println!(
                        "Sincronização concluída: {} pasta(s) e {} arquivo(s) importado(s) do disco ({}).",
                        stats.folders_imported,
                        stats.files_imported,
                        storage_root.display()
                    );
                }
                Err(e) => {
                    eprintln!("Erro ao sincronizar disco: {e:?}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Some("serve") | None => {}
        Some(other) => {
            eprintln!(
                "Comando desconhecido: '{other}'. Comandos disponíveis: serve, reset-password, sync-disk"
            );
            std::process::exit(2);
        }
    }

    if let Ok(stats) = sync_all_users(&storage_root, &users, &folders, &files).await {
        if stats.folders_imported > 0 || stats.files_imported > 0 {
            println!(
                "Sincronização inicial: {} pasta(s) e {} arquivo(s) importado(s) de {}",
                stats.folders_imported,
                stats.files_imported,
                storage_root.display()
            );
        }
    }

    let trash_days: i64 = env::var("TRASH_TTL_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|d: &i64| *d > 0)
        .unwrap_or(30);

    let store = FsStore::new(storage_root.clone());
    let trash_ttl = Duration::days(trash_days);

    let purge = PurgeExpired {
        users: Arc::new(users.clone()),
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
        users,
        files,
        folders,
        store,
        sessions: Default::default(),
        trash_ttl,
    };

    let app = http::router(state);

    let server_host: String = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let server_port = env::var("SERVER_PORT").unwrap_or_else(|_| "5000".to_string());
    let bind_addr = format!("{server_host}:{server_port}");
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("Falha ao iniciar o servidor em {bind_addr}: {e}"));

    println!(
        "Servidor inicializado em {bind_addr} (storage: {})",
        storage_root.display()
    );

    axum::serve(listener, app)
        .await
        .expect("Falha no servidor");
}
