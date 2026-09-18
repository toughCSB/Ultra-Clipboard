use anyhow::Context;
use sha2::{Digest, Sha384};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::ConnectOptions;
use sqlx::SqlitePool;
use tauri::AppHandle;

use crate::core::Result;
use crate::db::db_path;

pub async fn init(app: &AppHandle) -> Result<SqlitePool> {
    let path = db_path(app)?;

    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .disable_statement_logging();

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .with_context(|| format!("failed to open sqlite database at {path:?}"))?;

    let migrator = sqlx::migrate!("./migrations");
    repair_line_ending_checksum_drift(&pool, &migrator)
        .await
        .context("failed to reconcile migration checksums")?;

    migrator
        .run(&pool)
        .await
        .context("failed to run sqlite migrations")?;

    log::info!("sqlite pool ready at {path:?}");
    Ok(pool)
}

/// A migration file checked out with CRLF line endings (Windows, before
/// `.gitattributes` pinned `eol=lf` for `src-tauri/migrations/*.sql`) embeds
/// a different sqlx checksum than the same file checked out with LF, even
/// though the SQL itself is unchanged. sqlx treats that as the migration
/// having been edited after being applied and refuses to start.
///
/// Before sqlx's own strict check runs, resync any already-applied
/// migration's stored checksum when it matches the LF or CRLF variant of
/// what this build actually compiled, so a real edit is still caught while
/// this specific false positive self-heals.
async fn repair_line_ending_checksum_drift(
    pool: &SqlitePool,
    migrator: &sqlx::migrate::Migrator,
) -> Result<()> {
    let table_exists: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await
    .context("failed to check for the sqlx migrations table")?;
    if table_exists == 0 {
        return Ok(());
    }

    for migration in migrator.iter() {
        let current_checksum = migration.checksum.as_ref();

        let stored: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = ?")
                .bind(migration.version)
                .fetch_optional(pool)
                .await
                .with_context(|| {
                    format!(
                        "failed to read stored checksum for migration {}",
                        migration.version
                    )
                })?;

        let Some(stored_checksum) = stored else {
            continue;
        };
        if stored_checksum == current_checksum {
            continue;
        }

        let lf_only: Vec<u8> = migration
            .sql
            .as_ref()
            .bytes()
            .filter(|&byte| byte != b'\r')
            .collect();
        let mut crlf = Vec::with_capacity(lf_only.len());
        for &byte in &lf_only {
            if byte == b'\n' {
                crlf.push(b'\r');
            }
            crlf.push(byte);
        }

        let is_line_ending_drift = stored_checksum == Sha384::digest(&lf_only).as_slice()
            || stored_checksum == Sha384::digest(&crlf).as_slice();
        if !is_line_ending_drift {
            continue;
        }

        sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
            .bind(current_checksum)
            .bind(migration.version)
            .execute(pool)
            .await
            .with_context(|| {
                format!(
                    "failed to resync checksum for migration {}",
                    migration.version
                )
            })?;
        log::warn!(
            "migration {} checksum differed only by CRLF/LF line endings; resynced to the current build",
            migration.version
        );
    }

    Ok(())
}
