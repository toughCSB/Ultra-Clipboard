use std::time::{Duration, Instant};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use super::storage::ImageStore;
use super::watcher::CLIPBOARD_UPDATED_EVENT;
use crate::db::items::cleanup_history;
use crate::settings::{Retention, RetentionUnit, SettingsStore};

const SCHEDULER_TICK_INTERVAL: Duration = Duration::from_secs(60);

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        run_once(&app).await;
        let mut last_cleanup_at = Instant::now();
        let mut ticker = tokio::time::interval(SCHEDULER_TICK_INTERVAL);
        ticker.tick().await;

        loop {
            ticker.tick().await;
            let Some(interval) = cleanup_interval(&app) else {
                continue;
            };

            if last_cleanup_at.elapsed() < interval {
                continue;
            }

            run_once(&app).await;
            last_cleanup_at = Instant::now();
        }
    });
}

async fn run_once(app: &AppHandle) {
    let history = match app.try_state::<SettingsStore>() {
        Some(store) => store.snapshot().clipboard.history,
        None => return,
    };

    let cutoff = retention_cutoff(&history.retention, Utc::now());
    let max = (history.max_count > 0).then_some(history.max_count);

    if cutoff.is_none() && max.is_none() {
        return;
    }

    let pool = app.state::<crate::db::DatabaseState>().pool().await;
    match cleanup_history(&pool, cutoff, max).await {
        Ok(outcome) if outcome.removed == 0 => {}
        Ok(outcome) => {
            remove_images(app, &outcome.image_files);
            log::info!("history cleanup removed {} item(s)", outcome.removed);
            if let Err(err) = app.emit(
                CLIPBOARD_UPDATED_EVENT,
                json!({ "cleanup": outcome.removed }),
            ) {
                log::warn!("emit cleanup event failed: {err}");
            }
        }
        Err(err) => log::warn!("history cleanup failed: {err}"),
    }
}

fn cleanup_interval(app: &AppHandle) -> Option<Duration> {
    let store = app.try_state::<SettingsStore>()?;
    let hours = store.snapshot().clipboard.history.cleanup_interval_hours;

    if hours == 0 {
        return None;
    }

    Some(Duration::from_secs(u64::from(hours) * 60 * 60))
}

fn remove_images(app: &AppHandle, file_names: &[String]) {
    if file_names.is_empty() {
        return;
    }
    let Some(store) = app.try_state::<ImageStore>() else {
        log::warn!(
            "image store unavailable; skip removing {} image file(s)",
            file_names.len()
        );
        return;
    };
    for file_name in file_names {
        if let Err(err) = store.remove(file_name) {
            log::warn!("remove cleaned image {file_name} failed: {err}");
        }
    }
}

fn retention_cutoff(r: &Retention, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    if r.value == 0 {
        return None;
    }
    let dur = match r.unit {
        RetentionUnit::Forever => return None,
        RetentionUnit::Hours => ChronoDuration::hours(r.value as i64),
        RetentionUnit::Days => ChronoDuration::days(r.value as i64),
        RetentionUnit::Weeks => ChronoDuration::weeks(r.value as i64),
        RetentionUnit::Months => ChronoDuration::days((r.value as i64) * 30),
    };
    Some(now - dur)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).unwrap()
    }

    #[test]
    fn retention_cutoff_returns_none_when_disabled() {
        assert!(retention_cutoff(
            &Retention {
                value: 0,
                unit: RetentionUnit::Days
            },
            now()
        )
        .is_none());
        assert!(retention_cutoff(
            &Retention {
                value: 7,
                unit: RetentionUnit::Forever
            },
            now()
        )
        .is_none());
    }

    #[test]
    fn retention_cutoff_subtracts_by_unit() {
        let n = now();
        assert_eq!(
            retention_cutoff(
                &Retention {
                    value: 2,
                    unit: RetentionUnit::Hours
                },
                n
            ),
            Some(n - ChronoDuration::hours(2))
        );
        assert_eq!(
            retention_cutoff(
                &Retention {
                    value: 3,
                    unit: RetentionUnit::Days
                },
                n
            ),
            Some(n - ChronoDuration::days(3))
        );
        assert_eq!(
            retention_cutoff(
                &Retention {
                    value: 1,
                    unit: RetentionUnit::Weeks
                },
                n
            ),
            Some(n - ChronoDuration::weeks(1))
        );
        assert_eq!(
            retention_cutoff(
                &Retention {
                    value: 1,
                    unit: RetentionUnit::Months
                },
                n
            ),
            Some(n - ChronoDuration::days(30))
        );
    }
}
