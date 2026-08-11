//! Server database connection + migrations.
//!
//! Opens a SQLite pool with the server persistence PRAGMAs
//! (docs/architecture/server-db.md §2) and applies all pending migrations
//! from `migrations/`.

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous};
use std::str::FromStr;

/// Boxed error covering both `sqlx::Error` (connect) and `MigrateError` (schema).
pub type DbError = Box<dyn std::error::Error + Send + Sync>;

/// Open the server SQLite database and run migrations.
///
/// `db_url` is any `sqlite:` URL (file or `sqlite::memory:` for tests).
/// For `:memory:` pools we pin `max_connections(1)` so the single in-memory
/// database is shared across the pool (else each connection gets its own empty
/// memory DB).
pub async fn connect(db_url: &str) -> Result<SqlitePool, DbError> {
    let opts = SqliteConnectOptions::from_str(db_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_millis(5000));

    let pool = if db_url.contains(":memory:") {
        SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?
    } else {
        SqlitePoolOptions::new().connect_with(opts).await?
    };

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Convenience: run a migration check against a pool (used by tests/CI to assert
/// the schema is applied without booting the full server).
pub async fn ensure_schema(pool: &SqlitePool) -> Result<(), DbError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
