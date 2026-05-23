// media_core/src/db/base.rs
use sqlx::{sqlite::SqlitePool, FromRow, Executor};
use std::sync::Arc;
use crate::db::Result;

#[derive(Copy, Clone)]
pub struct TxPointer(pub *mut sqlx::SqliteConnection);
unsafe impl Send for TxPointer {}
unsafe impl Sync for TxPointer {}

tokio::task_local! {
    pub static ACTIVE_TX: Option<TxPointer>;
}

#[macro_export]
macro_rules! execute_db {
    ($pool:expr, $query:expr) => {
        async move {
            let conn = $crate::db::base::ACTIVE_TX.try_with(|tx| tx.map(|p| p.0)).ok().flatten();
            if let Some(conn_ptr) = conn {
                let conn = unsafe { &mut *conn_ptr };
                $query.execute(&mut *conn).await
            } else {
                $query.execute($pool).await
            }
        }
    };
}

#[macro_export]
macro_rules! fetch_one_db {
    ($pool:expr, $query:expr) => {
        async move {
            let conn = $crate::db::base::ACTIVE_TX.try_with(|tx| tx.map(|p| p.0)).ok().flatten();
            if let Some(conn_ptr) = conn {
                let conn = unsafe { &mut *conn_ptr };
                $query.fetch_one(&mut *conn).await
            } else {
                $query.fetch_one($pool).await
            }
        }
    };
}

#[macro_export]
macro_rules! fetch_optional_db {
    ($pool:expr, $query:expr) => {
        async move {
            let conn = $crate::db::base::ACTIVE_TX.try_with(|tx| tx.map(|p| p.0)).ok().flatten();
            if let Some(conn_ptr) = conn {
                let conn = unsafe { &mut *conn_ptr };
                $query.fetch_optional(&mut *conn).await
            } else {
                $query.fetch_optional($pool).await
            }
        }
    };
}

#[macro_export]
macro_rules! fetch_all_db {
    ($pool:expr, $query:expr) => {
        async move {
            let conn = $crate::db::base::ACTIVE_TX.try_with(|tx| tx.map(|p| p.0)).ok().flatten();
            if let Some(conn_ptr) = conn {
                let conn = unsafe { &mut *conn_ptr };
                $query.fetch_all(&mut *conn).await
            } else {
                $query.fetch_all($pool).await
            }
        }
    };
}

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
    pub async fn execute<'e, E>(&self, _executor: E, sql: &str, args: sqlx::sqlite::SqliteArguments<'e>) -> Result<()> 
    where E: Executor<'e, Database = sqlx::Sqlite>
    {
        let conn = ACTIVE_TX.try_with(|tx| tx.map(|p| p.0)).ok().flatten();
        if let Some(conn_ptr) = conn {
            let conn = unsafe { &mut *conn_ptr };
            sqlx::query_with(sql, args)
                .execute(conn)
                .await?;
        } else {
            sqlx::query_with(sql, args)
                .execute(&*self.pool)
                .await?;
        }
        Ok(())
    }

    /// Fetch all rows mapped to T.
    #[allow(dead_code)]
    pub async fn fetch_all<'e, T, E>(&self, _executor: E, sql: &str, args: sqlx::sqlite::SqliteArguments<'e>) -> Result<Vec<T>>
    where 
        T: for<'r> FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
        E: Executor<'e, Database = sqlx::Sqlite>
    {
        let conn = ACTIVE_TX.try_with(|tx| tx.map(|p| p.0)).ok().flatten();
        let rows = if let Some(conn_ptr) = conn {
            let conn = unsafe { &mut *conn_ptr };
            sqlx::query_as_with::<_, T, _>(sql, args)
                .fetch_all(conn)
                .await?
        } else {
            sqlx::query_as_with::<_, T, _>(sql, args)
                .fetch_all(&*self.pool)
                .await?
        };
        Ok(rows)
    }

    /// Fetch one optional row.
    #[allow(dead_code)]
    pub async fn fetch_optional<'e, T, E>(&self, _executor: E, sql: &str, args: sqlx::sqlite::SqliteArguments<'e>) -> Result<Option<T>>
    where 
        T: for<'r> FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
        E: Executor<'e, Database = sqlx::Sqlite>
    {
        let conn = ACTIVE_TX.try_with(|tx| tx.map(|p| p.0)).ok().flatten();
        let row = if let Some(conn_ptr) = conn {
            let conn = unsafe { &mut *conn_ptr };
            sqlx::query_as_with::<_, T, _>(sql, args)
                .fetch_optional(conn)
                .await?
        } else {
            sqlx::query_as_with::<_, T, _>(sql, args)
                .fetch_optional(&*self.pool)
                .await?
        };
        Ok(row)
    }

    /// Fetch one row.
    #[allow(dead_code)]
    pub async fn fetch_one<'e, T, E>(&self, _executor: E, sql: &str, args: sqlx::sqlite::SqliteArguments<'e>) -> Result<T>
    where 
        T: for<'r> FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
        E: Executor<'e, Database = sqlx::Sqlite>
    {
        let conn = ACTIVE_TX.try_with(|tx| tx.map(|p| p.0)).ok().flatten();
        let row = if let Some(conn_ptr) = conn {
            let conn = unsafe { &mut *conn_ptr };
            sqlx::query_as_with::<_, T, _>(sql, args)
                .fetch_one(conn)
                .await?
        } else {
            sqlx::query_as_with::<_, T, _>(sql, args)
                .fetch_one(&*self.pool)
                .await?
        };
        Ok(row)
    }
}
