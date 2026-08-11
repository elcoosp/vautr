//! Server database connection + migrations.
//!
//! SQLite is the default backend (docs/architecture/server-db.md §2). The pool
//! is opened with the server persistence PRAGMAs and all pending migrations
//! from `migrations/` are applied.
//!
//! # PostgreSQL (opt-in)
//!
//! A real PostgreSQL connection path is provided behind the `postgres` cargo
//! feature (see the `postgres` module below and [`connect_any`]). It is
//! **opt-in** so SQLite stays the zero-config default:
//!
//! 1. The sqlx `postgres` runtime feature must be enabled (coordinator action —
//!    this workstream owns neither `Cargo.toml` manifest).
//! 2. The `postgres` feature must be enabled on `vautr-server` (also a
//!    coordinator action).
//! 3. Point `VAUTR_DB_URL` at a `postgres://` URL, e.g.
//!    `postgres://vautr:vautr@localhost:5432/vautr`.
//!
//! `sqlx::migrate!` runs the *same* `migrations/*.sql` files on either backend.
//! Those files are currently written with SQLite DDL (`STRICT`, `BLOB`,
//! `TEXT PRIMARY KEY`, etc.), so enabling Postgres **also requires a set of
//! Postgres-compatible migrations** (the migrations directory is owned by
//! another workstream — see docs/SELF-HOSTING.md §"Database").

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous};
#[cfg(feature = "postgres")]
use sqlx::PgPool;
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

/// A unified handle over the supported server backends. This lets callers
/// dispatch on the `VAUTR_DB_URL` scheme at runtime without committing to a
/// concrete pool type until they have one.
///
/// The server `Repository` is currently SQLite-typed (it wraps `SqlitePool`).
/// Wiring it to consume the `Postgres` variant is a follow-up owned by the
/// repository workstream; this type exists so `db` exposes a single place where
/// backend selection happens.
pub enum AnyPool {
    /// SQLite (default).
    Sqlite(SqlitePool),
    /// PostgreSQL (requires the `postgres` cargo feature).
    #[cfg(feature = "postgres")]
    Postgres(PgPool),
}

/// Open a database by `VAUTR_DB_URL` scheme.
///
/// - `sqlite:` (or anything without `://`) → SQLite via [`connect`].
/// - `postgres://` / `postgresql://` → PostgreSQL via [`postgres::connect_pg`],
///   compiled only when the `postgres` feature is enabled.
///
/// Unknown schemes return an error so a typo'ed URL fails fast at startup.
pub async fn connect_any(db_url: &str) -> Result<AnyPool, DbError> {
    let scheme = db_url.split("://").next().unwrap_or("sqlite");
    match scheme {
        "sqlite" => Ok(AnyPool::Sqlite(connect(db_url).await?)),
        #[cfg(feature = "postgres")]
        "postgres" | "postgresql" => {
            Ok(AnyPool::Postgres(postgres::connect_pg(db_url).await?))
        }
        #[cfg(not(feature = "postgres"))]
        "postgres" | "postgresql" => Err(
            "PostgreSQL support is not compiled in: enable the vautr-server \
             `postgres` feature (see db.rs module docs and docs/SELF-HOSTING.md)"
                .into(),
        ),
        other => Err(format!(
            "unsupported database URL scheme: {other:?} \
             (expected `sqlite:` or, with the `postgres` feature, `postgres://`)"
        )
        .into()),
    }
}

/// PostgreSQL backend (opt-in). Compiles only when the `postgres` cargo feature
/// is enabled, so the default SQLite build is unaffected.
#[cfg(feature = "postgres")]
pub mod postgres {
    use super::DbError;
    use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
    use std::str::FromStr;

    /// Open a PostgreSQL pool from a `postgres://` URL and run all pending
    /// migrations. `sslmode` etc. are parsed from the URL by
    /// [`PgConnectOptions::from_str`].
    ///
    /// # Backend compatibility
    ///
    /// `sqlx::migrate!` applies the shared `migrations/*.sql`. Those files are
    /// currently SQLite-specific DDL (`STRICT`, `BLOB`, …), so this call will
    /// only succeed against a database whose schema was created from
    /// Postgres-compatible migrations. See the module-level docs and
    /// docs/SELF-HOSTING.md §"Database".
    pub async fn connect_pg(db_url: &str) -> Result<PgPool, DbError> {
        let opts = PgConnectOptions::from_str(db_url)?;
        let pool = PgPoolOptions::new().connect_with(opts).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(pool)
    }

    /// Convenience: run a migration check against a Postgres pool.
    pub async fn ensure_schema_pg(pool: &PgPool) -> Result<(), DbError> {
        sqlx::migrate!("./migrations").run(pool).await?;
        Ok(())
    }
}
