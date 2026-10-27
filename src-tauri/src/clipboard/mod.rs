mod app_store;
mod apps_registry;
mod cleanup;
mod detect;
mod file_icon_store;
mod guard;
mod icon;
mod ingest;
pub mod item_policy;
pub mod item_view;
pub mod paste;
mod payload;
mod read;
mod secrets;
mod sound;
mod source;
mod storage;
mod updated;
mod watcher;
mod write;

pub use app_store::AppIconStore;
pub use apps_registry::{
    add_app_from_path, delete_unreferenced_apps, refresh_running_apps, AppsRegistry,
};
pub use detect::sanitize_css_color;
pub use file_icon_store::FileIconStore;
pub use guard::WritebackGuard;
pub use icon::{get_icon_cache_key, icon_png, DIR_CACHE_KEY};
#[cfg(test)]
pub use ingest::build_item;
pub use ingest::build_item_with_settings;
pub use payload::{ClipboardPayload, ImagePayload, TextPayload};
pub use sound::{maybe_play_copy, play_copy_sound_now};
pub use storage::{validate_image_file_name, ImageStore};
pub use updated::{emit_clipboard_updated, ClipboardUpdated};
pub use watcher::{init, persist_and_notify, WatcherPause};
pub use write::write_to_clipboard;

#[cfg(test)]
pub(crate) mod test_lock {
    use std::sync::{Mutex, MutexGuard};

    static LOCK: Mutex<()> = Mutex::new(());

    pub fn serial() -> MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
