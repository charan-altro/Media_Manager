// media_core/src/db/library_repo.rs
use crate::models::{Library, LibraryId, MediaType};
use crate::db::Result;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;

// --- Reader interface (read-only consumers get this) ---
#[cfg_attr(test, mockall::automock)]
#[allow(async_fn_in_trait)]
pub trait LibraryReader: Send + Sync {
    async fn find_all(&self) -> Result<Vec<Library>>;
    async fn find_by_id(&self, id: LibraryId) -> Result<Option<Library>>;
}

// --- Writer interface (write consumers get this) ---
#[cfg_attr(test, mockall::automock)]
#[allow(async_fn_in_trait)]
pub trait LibraryWriter: Send + Sync {
    async fn insert(&self, name: &str, path: &str, media_type: MediaType) -> Result<LibraryId>;
    async fn delete(&self, id: LibraryId) -> Result<()>;
}

// --- Combined (most services need both) ---
pub trait LibraryReaderWriter: LibraryReader + LibraryWriter {}

// --- SQLite implementation ---
pub struct SqliteLibraryRepository {
    base: super::base::SqliteBase,
}

impl SqliteLibraryRepository {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self {
            base: super::base::SqliteBase::new(pool),
        }
    }
}

impl LibraryReader for SqliteLibraryRepository {
    #[tracing::instrument(skip(self), err)]
    async fn find_all(&self) -> Result<Vec<Library>> {
        let sql = r#"
            SELECT id, name, path, media_type, created_at
            FROM libraries
        "#;
        self.base.fetch_all(&*self.base.pool, sql, sqlx::sqlite::SqliteArguments::default()).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_by_id(&self, id: LibraryId) -> Result<Option<Library>> {
        let sql = "SELECT * FROM libraries WHERE id = ?";
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, id);
        self.base.fetch_optional(&*self.base.pool, sql, args).await
    }
}

impl LibraryWriter for SqliteLibraryRepository {
    #[tracing::instrument(skip(self), err)]
    async fn insert(&self, name: &str, path: &str, media_type: MediaType) -> Result<LibraryId> {
        let mt_str = match media_type {
            MediaType::Movie => "movie",
            MediaType::Tv => "tv",
        };
        
        let normalized_path = crate::paths::normalize_slashes(path);
        
        sqlx::query(
            r#"
            INSERT INTO libraries (name, path, media_type)
            VALUES (?, ?, ?)
            ON CONFLICT(path) DO UPDATE SET name=excluded.name
            "#
        )
        .bind(name)
        .bind(&normalized_path)
        .bind(mt_str)
        .execute(&*self.base.pool)
        .await?;

        let row: (LibraryId,) = sqlx::query_as("SELECT id FROM libraries WHERE path = ?")
            .bind(&normalized_path)
            .fetch_one(&*self.base.pool)
            .await?;
        
        Ok(row.0)
    }

    #[tracing::instrument(skip(self), err)]
    async fn delete(&self, id: LibraryId) -> Result<()> {
        sqlx::query("DELETE FROM libraries WHERE id = ?")
            .bind(id)
            .execute(&*self.base.pool)
            .await?;
        Ok(())
    }
}

impl LibraryReaderWriter for SqliteLibraryRepository {}
