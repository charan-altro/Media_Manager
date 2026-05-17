// media_core/src/db/base.rs
use sqlx::{sqlite::SqlitePool, FromRow, Executor};
use std::sync::Arc;
use crate::db::Result;

/// Shared infrastructure embedded by all concrete Sqlite*Repository types.
/// Not public — callers only see the trait interfaces.
pub(super) struct SqliteBase {
    pub pool: Arc<SqlitePool>,
}

impl SqliteBase {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }

    /// Execute a query that returns no rows.
    #[allow(dead_code)]
    pub async fn execute<'e, E>(&self, executor: E, sql: &str, args: sqlx::sqlite::SqliteArguments<'e>) -> Result<()> 
    where E: Executor<'e, Database = sqlx::Sqlite>
    {
        sqlx::query_with(sql, args)
            .execute(executor)
            .await?;
        Ok(())
    }

    /// Fetch all rows mapped to T.
    #[allow(dead_code)]
    pub async fn fetch_all<'e, T, E>(&self, executor: E, sql: &str, args: sqlx::sqlite::SqliteArguments<'e>) -> Result<Vec<T>>
    where 
        T: for<'r> FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
        E: Executor<'e, Database = sqlx::Sqlite>
    {
        let rows = sqlx::query_as_with::<_, T, _>(sql, args)
            .fetch_all(executor)
            .await?;
        Ok(rows)
    }

    /// Fetch one optional row.
    #[allow(dead_code)]
    pub async fn fetch_optional<'e, T, E>(&self, executor: E, sql: &str, args: sqlx::sqlite::SqliteArguments<'e>) -> Result<Option<T>>
    where 
        T: for<'r> FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
        E: Executor<'e, Database = sqlx::Sqlite>
    {
        let row = sqlx::query_as_with::<_, T, _>(sql, args)
            .fetch_optional(executor)
            .await?;
        Ok(row)
    }

    /// Fetch one row.
    #[allow(dead_code)]
    pub async fn fetch_one<'e, T, E>(&self, executor: E, sql: &str, args: sqlx::sqlite::SqliteArguments<'e>) -> Result<T>
    where 
        T: for<'r> FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
        E: Executor<'e, Database = sqlx::Sqlite>
    {
        let row = sqlx::query_as_with::<_, T, _>(sql, args)
            .fetch_one(executor)
            .await?;
        Ok(row)
    }
}
