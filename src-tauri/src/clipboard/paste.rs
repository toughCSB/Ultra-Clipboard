//! Copying or pasting a stored item: the clipboard write, the reuse bump, and
//! the window and keystroke sequence a paste needs.

use std::time::Duration;

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use crate::clipboard::{emit_clipboard_updated, ClipboardUpdated, ImageStore, WritebackGuard};
use crate::core::{AppError, Result};
use crate::db::items::{find_item_by_id, increment_item_use_count};
use crate::db::models::{ClipboardItem, ClipboardKind};
use crate::settings::SettingsStore;
use crate::window::{self, CLIPBOARD_WINDOW_LABEL};

/// Time the target app needs to take focus back before the keystroke lands.
const FOCUS_SETTLE: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PasteStep {
    HideWindow,
    ResignPanelKey,
    Wait,
    SimulatePaste,
    RestorePanelKey,
}

/// A pinned window stays open, so the macOS panel gives up key focus for the
/// keystroke and takes it back afterwards.
fn paste_steps(pinned: bool, macos: bool) -> Vec<PasteStep> {
    use PasteStep::*;

    match (pinned, macos) {
        (false, _) => vec![HideWindow, Wait, SimulatePaste],
        (true, true) => vec![ResignPanelKey, Wait, SimulatePaste, Wait, RestorePanelKey],
        (true, false) => vec![Wait, SimulatePaste],
    }
}

pub async fn copy_item(
    app: &AppHandle,
    pool: &SqlitePool,
    store: &ImageStore,
    guard: &WritebackGuard,
    id: &str,
    force_plain: bool,
) -> Result<()> {
    let settings = app.state::<SettingsStore>().snapshot();
    let item = find_item(pool, id).await?;
    let content = &settings.clipboard.content;
    let plain = should_write_plain_for_copy(force_plain, item.kind, content.copy_plain);

    write_item(
        app,
        pool,
        store,
        guard,
        &item,
        plain,
        content.update_on_reuse,
    )
    .await?;

    if content.copy_then_hide_window && !window::is_clipboard_window_pinned() {
        if let Err(err) = window::hide_window(app, CLIPBOARD_WINDOW_LABEL) {
            log::warn!("hide clipboard window after copy failed: {err:?}");
        }
    }

    Ok(())
}

pub async fn paste_item(
    app: &AppHandle,
    pool: &SqlitePool,
    store: &ImageStore,
    guard: &WritebackGuard,
    id: &str,
    force_plain: bool,
) -> Result<()> {
    let settings = app.state::<SettingsStore>().snapshot();
    let item = find_item(pool, id).await?;
    let content = &settings.clipboard.content;
    let plain = should_write_plain_for_paste(
        force_plain,
        item.kind,
        content.paste_plain,
        content.paste_files_as_path,
    );

    write_item(
        app,
        pool,
        store,
        guard,
        &item,
        plain,
        content.update_on_reuse,
    )
    .await?;

    let steps = paste_steps(
        window::is_clipboard_window_pinned(),
        cfg!(target_os = "macos"),
    );
    for step in steps {
        run_step(app, step).await?;
    }

    Ok(())
}

async fn run_step(app: &AppHandle, step: PasteStep) -> Result<()> {
    match step {
        PasteStep::HideWindow => {
            if let Err(err) = window::hide_window(app, CLIPBOARD_WINDOW_LABEL) {
                log::warn!("hide clipboard window before paste failed: {err:?}");
            }
        }
        PasteStep::Wait => tokio::time::sleep(FOCUS_SETTLE).await,
        PasteStep::SimulatePaste => crate::keystroke::simulate_paste()?,
        #[cfg(target_os = "macos")]
        PasteStep::ResignPanelKey => {
            if let Err(err) = window::macos::resign_clipboard_panel_key(app) {
                log::warn!("resign clipboard panel key before paste failed: {err:?}");
            }
        }
        #[cfg(target_os = "macos")]
        PasteStep::RestorePanelKey => {
            if let Err(err) = window::macos::make_clipboard_panel_key(app) {
                log::warn!("restore clipboard panel key after paste failed: {err:?}");
            }
        }
        #[cfg(not(target_os = "macos"))]
        PasteStep::ResignPanelKey | PasteStep::RestorePanelKey => {}
    }

    Ok(())
}

async fn find_item(pool: &SqlitePool, id: &str) -> Result<ClipboardItem> {
    find_item_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::Clipboard(format!("clipboard item not found: {id}")))
}

async fn write_item(
    app: &AppHandle,
    pool: &SqlitePool,
    store: &ImageStore,
    guard: &WritebackGuard,
    item: &ClipboardItem,
    plain: bool,
    update_on_reuse: bool,
) -> Result<()> {
    crate::clipboard::write_to_clipboard(store, guard, item, plain)?;

    if !update_on_reuse {
        return Ok(());
    }

    increment_item_use_count(pool, &item.id).await?;
    emit_clipboard_updated(
        app,
        ClipboardUpdated::Captured {
            id: &item.id,
            kind: item.kind,
            deduplicated: true,
        },
    );

    Ok(())
}

fn should_write_plain_for_copy(force_plain: bool, kind: ClipboardKind, copy_plain: bool) -> bool {
    force_plain || kind == ClipboardKind::Text && copy_plain
}

fn should_write_plain_for_paste(
    force_plain: bool,
    kind: ClipboardKind,
    paste_plain: bool,
    paste_files_as_path: bool,
) -> bool {
    force_plain
        || kind == ClipboardKind::Text && paste_plain
        || kind == ClipboardKind::Files && paste_files_as_path
}

#[cfg(test)]
mod tests {
    use super::PasteStep::*;
    use super::*;

    #[test]
    fn unpinned_paste_hides_the_window_before_the_keystroke() {
        assert_eq!(
            paste_steps(false, true),
            vec![HideWindow, Wait, SimulatePaste]
        );
        assert_eq!(
            paste_steps(false, false),
            vec![HideWindow, Wait, SimulatePaste]
        );
    }

    #[test]
    fn pinned_macos_paste_hands_key_focus_back_after_the_keystroke() {
        assert_eq!(
            paste_steps(true, true),
            vec![ResignPanelKey, Wait, SimulatePaste, Wait, RestorePanelKey]
        );
    }

    #[test]
    fn pinned_windows_paste_keeps_the_window_and_only_waits() {
        assert_eq!(paste_steps(true, false), vec![Wait, SimulatePaste]);
    }

    #[test]
    fn copy_plain_default_only_affects_text_items() {
        assert!(should_write_plain_for_copy(
            false,
            ClipboardKind::Text,
            true
        ));
        assert!(!should_write_plain_for_copy(
            false,
            ClipboardKind::Files,
            true
        ));
        assert!(!should_write_plain_for_copy(
            false,
            ClipboardKind::Image,
            true
        ));
        assert!(!should_write_plain_for_copy(
            false,
            ClipboardKind::Text,
            false
        ));
    }

    #[test]
    fn force_plain_copy_overrides_item_kind() {
        assert!(should_write_plain_for_copy(
            true,
            ClipboardKind::Files,
            false
        ));
        assert!(should_write_plain_for_copy(
            true,
            ClipboardKind::Image,
            false
        ));
    }

    #[test]
    fn paste_plain_defaults_follow_item_kind() {
        assert!(should_write_plain_for_paste(
            false,
            ClipboardKind::Text,
            true,
            false,
        ));
        assert!(should_write_plain_for_paste(
            false,
            ClipboardKind::Files,
            false,
            true,
        ));
        assert!(!should_write_plain_for_paste(
            false,
            ClipboardKind::Files,
            true,
            false,
        ));
        assert!(!should_write_plain_for_paste(
            false,
            ClipboardKind::Image,
            true,
            true,
        ));
    }

    #[test]
    fn force_plain_paste_overrides_item_kind() {
        assert!(should_write_plain_for_paste(
            true,
            ClipboardKind::Image,
            false,
            false,
        ));
    }
}
