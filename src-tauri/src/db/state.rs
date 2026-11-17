use std::future::Future;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::core::Result;

/// Holds the current SQLite pool and serializes hot replacement during backup overwrite import.
pub struct DatabaseState {
    pool: Mutex<SqlitePool>,
}

impl DatabaseState {
    /// Creates database state from the startup pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool: Mutex::new(pool),
        }
    }

    /// Returns a clone of the currently active pool.
    pub async fn pool(&self) -> SqlitePool {
        self.pool.lock().await.clone()
    }

    /// Closes the active pool, runs replacement work, and installs the new pool atomically.
    pub async fn close_and_replace<F, Fut>(&self, replace: F) -> Result<()>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<SqlitePool>>,
    {
        let mut current = self.pool.lock().await;
        current.close().await;
        let next = replace().await?;
        *current = next;
        Ok(())
    }

    /// Reopens a usable pool after a failed replacement while the pool lock is held.
    pub async fn close_and_replace_with_recovery<F, Fut, R, RFut>(
        &self,
        replace: F,
        mut recover: R,
    ) -> Result<()>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<SqlitePool>>,
        R: FnMut() -> RFut,
        RFut: Future<Output = Result<SqlitePool>>,
    {
        let mut current = self.pool.lock().await;
        current.close().await;
        match replace().await {
            Ok(next) => {
                *current = next;
                Ok(())
            }
            Err(err) => match recover().await {
                Ok(previous) => {
                    *current = previous;
                    Err(err)
                }
                Err(recovery_err) => Err(anyhow::anyhow!(
                    "{err}; restoring the previous database also failed: {recovery_err}"
                )
                .into()),
            },
        }
    }

    /// Also rolls back a successfully installed pool if required finalization fails.
    pub async fn replace_with_finalize<F, Fut, R, RFut, V, VFut>(
        &self,
        install: F,
        mut recover: R,
        finalize: V,
    ) -> Result<()>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<SqlitePool>>,
        R: FnMut() -> RFut,
        RFut: Future<Output = Result<SqlitePool>>,
        V: FnOnce() -> VFut,
        VFut: Future<Output = Result<()>>,
    {
        self.close_and_replace_with_recovery(install, &mut recover)
            .await?;
        if let Err(err) = finalize().await {
            if let Err(recovery_err) = self.close_and_replace(recover).await {
                return Err(anyhow::anyhow!(
                    "{err}; restoring the previous database also failed: {recovery_err}"
                )
                .into());
            }
            return Err(err);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn open(path: &std::path::Path) -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn failed_replacement_reopens_previous_database() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("original.db");
        let initial = open(&path).await;
        sqlx::query("CREATE TABLE marker (value INTEGER NOT NULL)")
            .execute(&initial)
            .await
            .unwrap();
        sqlx::query("INSERT INTO marker VALUES (42)")
            .execute(&initial)
            .await
            .unwrap();
        let state = DatabaseState::new(initial);

        let error = state
            .close_and_replace_with_recovery(
                || async {
                    Err(crate::core::AppError::Clipboard(
                        "replacement refused".into(),
                    ))
                },
                || async { Ok(open(&path).await) },
            )
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), "replacement refused");
        let restored = state.pool().await;
        assert!(!restored.is_closed());
        let value: i64 = sqlx::query_scalar("SELECT value FROM marker")
            .fetch_one(&restored)
            .await
            .unwrap();
        assert_eq!(value, 42);
    }

    #[tokio::test]
    async fn failed_recovery_reports_both_errors() {
        let state = DatabaseState::new(crate::db::test_support::memory_pool().await);
        let error = state
            .close_and_replace_with_recovery(
                || async {
                    Err(crate::core::AppError::Clipboard(
                        "replacement refused".into(),
                    ))
                },
                || async { Err(crate::core::AppError::Clipboard("reopen refused".into())) },
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("replacement refused"));
        assert!(error.to_string().contains("reopen refused"));
        assert!(state.pool().await.is_closed());
    }

    #[tokio::test]
    async fn finalization_error_restores_previous_pool_and_rows() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("old.db");
        let incoming = temp.path().join("incoming.db");
        let initial = open(&old).await;
        sqlx::query("CREATE TABLE marker (value INTEGER NOT NULL)")
            .execute(&initial)
            .await
            .unwrap();
        sqlx::query("INSERT INTO marker VALUES (17)")
            .execute(&initial)
            .await
            .unwrap();
        let candidate = open(&incoming).await;
        sqlx::query("CREATE TABLE marker (value INTEGER NOT NULL)")
            .execute(&candidate)
            .await
            .unwrap();
        sqlx::query("INSERT INTO marker VALUES (99)")
            .execute(&candidate)
            .await
            .unwrap();
        candidate.close().await;
        let state = DatabaseState::new(initial);

        let error = state
            .replace_with_finalize(
                || async { Ok(open(&incoming).await) },
                || async { Ok(open(&old).await) },
                || async { Err(crate::core::AppError::Clipboard("settings rejected".into())) },
            )
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), "settings rejected");
        let pool = state.pool().await;
        let value: i64 = sqlx::query_scalar("SELECT value FROM marker")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(value, 17);
    }
}
