//! Operations the frontend may call, grouped by the window kind that owns them.
//! Each group checks the caller's window label and keeps operations that touch
//! windows on the main thread, in order with capture and timeout callbacks.

use tauri::ipc::Request;
use tauri::{AppHandle, Manager};

use super::state::ScreenshotState;
use super::{
    on_main_thread, output, pin, translation, CaptureMode, ExportResult, ImageInfo, OverlayState,
    PixelRect, TranslationStatus, EDITOR_WINDOW_LABEL_PREFIX, OVERLAY_WINDOW_LABEL_PREFIX,
    PIN_WINDOW_LABEL_PREFIX,
};
use crate::core::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowKind {
    Overlay,
    Editor,
    Pin,
}

impl WindowKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Overlay => OVERLAY_WINDOW_LABEL_PREFIX,
            Self::Editor => EDITOR_WINDOW_LABEL_PREFIX,
            Self::Pin => PIN_WINDOW_LABEL_PREFIX,
        }
    }
}

const OVERLAY_ONLY: &[WindowKind] = &[WindowKind::Overlay];
const EDITOR_ONLY: &[WindowKind] = &[WindowKind::Editor];
const PIN_ONLY: &[WindowKind] = &[WindowKind::Pin];
const IMAGE_WINDOWS: &[WindowKind] = &[WindowKind::Editor, WindowKind::Pin];

fn require(label: &str, kinds: &[WindowKind]) -> Result<()> {
    if kinds.iter().any(|kind| label.starts_with(kind.prefix())) {
        return Ok(());
    }

    Err(AppError::Other(anyhow::anyhow!(
        "screenshot window label is invalid"
    )))
}

fn closed() -> AppError {
    AppError::Other(anyhow::anyhow!("screenshot window is closed"))
}

/// Operations without a window label.
pub mod any {
    use super::*;

    pub fn start_capture(app: &AppHandle, mode: CaptureMode) {
        super::super::start_capture(app, mode);
    }

    pub async fn cancel_capture(app: &AppHandle, session_id: Option<u64>) -> Result<()> {
        on_main_thread(app, move |app| {
            super::super::cancel_capture(&app, session_id);
        })
        .await
    }

    pub async fn translation_status() -> Result<TranslationStatus> {
        translation::status().await
    }

    pub async fn set_translation_key(key: String) -> Result<()> {
        translation::set_key(key).await
    }

    pub async fn remove_translation_key() -> Result<()> {
        translation::remove_key().await
    }

    pub async fn copy_color(color: &str) -> Result<()> {
        output::copy_color(color).await
    }

    pub async fn read_clipboard_image() -> Result<Vec<u8>> {
        output::read_clipboard_png().await
    }
}

/// Operations for selection overlay windows.
pub mod overlay {
    use super::*;

    pub fn state(app: &AppHandle, label: &str) -> Result<Option<OverlayState>> {
        require(label, OVERLAY_ONLY)?;

        Ok(super::super::overlay_state(app, label))
    }

    pub async fn frame(app: &AppHandle, label: &str, session_id: u64) -> Result<Vec<u8>> {
        require(label, OVERLAY_ONLY)?;

        super::super::overlay_frame(app, label, session_id).await
    }

    pub async fn ready(app: &AppHandle, label: String, session_id: u64) -> Result<()> {
        require(&label, OVERLAY_ONLY)?;

        on_main_thread(app, move |app| {
            super::super::overlay::reveal(&app, &label, session_id)
        })
        .await?
    }

    pub async fn commit_selection(
        app: &AppHandle,
        label: String,
        session_id: u64,
        rect: PixelRect,
    ) -> Result<()> {
        require(&label, OVERLAY_ONLY)?;

        on_main_thread(app, move |app| {
            super::super::commit_selection(&app, &label, session_id, rect)
        })
        .await?
    }
}

/// Operations for editor and pin windows, which both hold a captured image.
pub mod image_window {
    use super::*;

    pub fn info(app: &AppHandle, label: &str) -> Result<ImageInfo> {
        require(label, IMAGE_WINDOWS)?;

        app.state::<ScreenshotState>()
            .image(label)
            .map(|image| ImageInfo::from_image(&image))
            .ok_or_else(closed)
    }

    pub fn pixels(app: &AppHandle, label: &str) -> Result<Vec<u8>> {
        require(label, IMAGE_WINDOWS)?;

        app.state::<ScreenshotState>()
            .image(label)
            .map(|image| image.rgba.clone())
            .ok_or_else(closed)
    }

    pub async fn ready(app: &AppHandle, label: String) -> Result<()> {
        require(&label, IMAGE_WINDOWS)?;

        on_main_thread(app, move |app| {
            super::super::reveal_painted_window(&app, &label)
        })
        .await?
    }

    pub async fn close(app: &AppHandle, label: String) -> Result<()> {
        require(&label, IMAGE_WINDOWS)?;

        on_main_thread(app, move |app| {
            super::super::close_window(&app, &label);
        })
        .await
    }

    pub async fn export(app: &AppHandle, request: &Request<'_>) -> Result<ExportResult> {
        let export = output::parse_request(request)?;
        require(&export.label, IMAGE_WINDOWS)?;

        super::super::export(app, export).await
    }
}

/// Operations for editor windows.
pub mod editor_window {
    use super::*;

    pub fn cancel_translation(app: &AppHandle, label: &str, request_id: u64) -> Result<()> {
        require(label, EDITOR_ONLY)?;
        app.state::<translation::TranslationFlights>()
            .cancel(label, request_id);

        Ok(())
    }

    pub async fn start_drag(app: &AppHandle, label: &str) -> Result<()> {
        require(label, EDITOR_ONLY)?;

        output::start_drag(app, label).await
    }

    pub async fn copy_text(app: &AppHandle, label: &str, text: &str) -> Result<()> {
        require(label, EDITOR_ONLY)?;
        if app.state::<ScreenshotState>().image(label).is_none() {
            return Err(closed());
        }

        output::copy_text(text).await
    }
}

/// Operations for pinned image windows.
pub mod pin_window {
    use super::*;

    pub async fn set_scale(
        app: &AppHandle,
        label: String,
        scale: f64,
        anchor_x: f64,
        anchor_y: f64,
    ) -> Result<()> {
        require(&label, PIN_ONLY)?;

        on_main_thread(app, move |app| {
            pin::set_scale(&app, &label, scale, anchor_x, anchor_y)
        })
        .await?
    }

    pub async fn show_menu(app: &AppHandle, label: String) -> Result<()> {
        require(&label, PIN_ONLY)?;

        on_main_thread(app, move |app| pin::show_menu(&app, &label)).await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OVERLAY: &str = "screenshot-overlay-0";
    const EDITOR: &str = "screenshot-editor-1";
    const PIN: &str = "screenshot-pin-2";

    fn accepted(kinds: &[WindowKind]) -> Vec<&'static str> {
        [OVERLAY, EDITOR, PIN]
            .into_iter()
            .filter(|label| require(label, kinds).is_ok())
            .collect()
    }

    #[test]
    fn overlay_group_accepts_only_overlay_labels() {
        assert_eq!(accepted(OVERLAY_ONLY), [OVERLAY]);
    }

    #[test]
    fn editor_group_accepts_only_editor_labels() {
        assert_eq!(accepted(EDITOR_ONLY), [EDITOR]);
    }

    #[test]
    fn pin_group_accepts_only_pin_labels() {
        assert_eq!(accepted(PIN_ONLY), [PIN]);
    }

    #[test]
    fn image_window_group_accepts_editor_and_pin_labels() {
        assert_eq!(accepted(IMAGE_WINDOWS), [EDITOR, PIN]);
    }

    #[test]
    fn foreign_labels_are_rejected_with_the_public_message() {
        for label in ["main", "", "preference", "screenshot-editor"] {
            let err = require(label, IMAGE_WINDOWS).unwrap_err();

            assert!(err
                .to_string()
                .contains("screenshot window label is invalid"));
            assert!(require(label, OVERLAY_ONLY).is_err());
        }
    }
}
