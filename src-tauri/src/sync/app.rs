use std::sync::Arc;

use tauri::{AppHandle, Manager};

use super::receiver::AppEventSink;
use super::runtime::{RuntimeInput, SyncRuntime};
use super::secrets::OsSecretStore;
use crate::clipboard::ImageStore;
use crate::core::Result;
use crate::settings::SettingsStore;

pub async fn init(app: &AppHandle) -> Result<()> {
    let runtime = SyncRuntime::new(Arc::new(OsSecretStore::default()));
    app.manage(runtime.clone());
    configure_app(&runtime, app).await
}

pub async fn reconfigure(app: &AppHandle) -> Result<()> {
    let runtime = app.state::<SyncRuntime>().inner().clone();
    configure_app(&runtime, app).await
}

async fn configure_app(runtime: &SyncRuntime, app: &AppHandle) -> Result<()> {
    let settings = app.state::<SettingsStore>().snapshot();
    let pool = app.state::<crate::db::DatabaseState>().pool().await;
    runtime
        .configure(
            RuntimeInput {
                settings: settings.sync,
                pool,
                image_store: app.state::<ImageStore>().inner().clone(),
                sensitive: settings.clipboard.sensitive,
                sink: Arc::new(AppEventSink::new(app.clone())),
            },
            false,
        )
        .await
}
