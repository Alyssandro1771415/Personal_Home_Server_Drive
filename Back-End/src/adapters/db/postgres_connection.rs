use sqlx::PgPool;

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("{key} não definida no .env"))
}

pub async fn postgres_connection() -> Result<PgPool, sqlx::Error> {
    let user = env("DATABASE_USER");
    let password = env("DATABASE_PASSWORD");
    let host = env("DATABASE_HOST");
    let port = env("DATABASE_PORT");
    let name = env("DATABASE_NAME");

    let database_url = format!("postgres://{user}:{password}@{host}:{port}/{name}");

    PgPool::connect(&database_url).await
}
