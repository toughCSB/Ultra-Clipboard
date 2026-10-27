//! In-memory screenshot state: the active capture session and the images owned
//! by editor and pin windows. Nothing here is persisted.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, Local};

use super::geometry::PixelRect;
use super::CaptureMode;

#[derive(Debug, Clone, Copy)]
pub struct MonitorGeometry {
    /// Monitor bounds in virtual desktop physical pixels.
    pub bounds: PixelRect,
    pub work_area: PixelRect,
    pub scale_factor: f64,
}

pub struct MonitorFrame {
    pub label: String,
    pub monitor: MonitorGeometry,
    /// `None` while an overlay shown before the capture waits for its pixels.
    pub rgba: Option<Arc<Vec<u8>>>,
}

pub struct CaptureSession {
    pub id: u64,
    pub mode: CaptureMode,
    pub frames: Vec<MonitorFrame>,
    /// Capturable windows in virtual desktop pixels, front to back.
    pub windows: Vec<PixelRect>,
    pub cursor_label: String,
    pub previous_foreground: Option<isize>,
    pub captured_at: DateTime<Local>,
}

impl CaptureSession {
    pub fn frame(&self, label: &str) -> Option<&MonitorFrame> {
        self.frames.iter().find(|frame| frame.label == label)
    }

    /// Whether every monitor's pixels have arrived.
    pub fn is_ready(&self) -> bool {
        self.frames.iter().all(|frame| frame.rgba.is_some())
    }
}

pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    /// Virtual desktop position of the image's top-left pixel when it was captured.
    pub origin_x: i32,
    pub origin_y: i32,
    pub rgba: Vec<u8>,
    pub captured_at: DateTime<Local>,
}

#[derive(Default)]
pub struct ScreenshotState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    capturing: bool,
    session: Option<CaptureSession>,
    handoff_editor: Option<String>,
    images: HashMap<String, Arc<CapturedImage>>,
    drag_files: HashMap<String, DragFile>,
    /// Last confirmed area in virtual desktop pixels, reused by repeat capture.
    last_area: Option<PixelRect>,
}

#[derive(Clone)]
pub struct DragFile {
    pub path: std::path::PathBuf,
    pub preview_png: Option<Vec<u8>>,
}

impl ScreenshotState {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| {
            log::error!("screenshot state mutex poisoned, recovering");
            poisoned.into_inner()
        })
    }

    pub fn next_id(&self) -> u64 {
        let mut inner = self.lock();
        inner.next_id += 1;

        inner.next_id
    }

    /// Marks a capture as in progress. Returns `false` when one is already running.
    pub fn try_begin_capture(&self) -> bool {
        let mut inner = self.lock();
        if inner.capturing {
            return false;
        }

        inner.capturing = true;
        true
    }

    pub fn finish_capture(&self) {
        let mut inner = self.lock();
        inner.capturing = false;
        inner.session = None;
        inner.handoff_editor = None;
    }

    pub fn begin_editor_handoff(&self, label: &str) {
        let mut inner = self.lock();
        inner.capturing = true;
        inner.handoff_editor = Some(label.to_owned());
    }

    pub fn complete_editor_handoff(&self, label: &str) -> bool {
        let mut inner = self.lock();
        if inner.handoff_editor.as_deref() != Some(label) {
            return false;
        }

        inner.handoff_editor = None;
        inner.capturing = false;
        true
    }

    pub fn set_session(&self, session: CaptureSession) {
        self.lock().session = Some(session);
    }

    pub fn with_session<R>(&self, read: impl FnOnce(&CaptureSession) -> R) -> Option<R> {
        self.lock().session.as_ref().map(read)
    }

    /// Stores the pixels and window list of a session that was shown before its
    /// capture. Returns `false` when the session was cancelled meanwhile.
    pub fn fill_session(&self, id: u64, pixels: Vec<Vec<u8>>, windows: Vec<PixelRect>) -> bool {
        let mut inner = self.lock();
        let Some(session) = inner.session.as_mut().filter(|session| session.id == id) else {
            return false;
        };

        for (frame, rgba) in session.frames.iter_mut().zip(pixels) {
            frame.rgba = Some(Arc::new(rgba));
        }
        session.windows = windows;
        true
    }

    /// Ends the capture and hands the session to the caller when `id` is still current.
    pub fn take_session(&self, id: Option<u64>) -> Option<CaptureSession> {
        let mut inner = self.lock();
        let matches = inner
            .session
            .as_ref()
            .is_some_and(|session| id.is_none_or(|id| session.id == id));
        if !matches {
            return None;
        }

        inner.capturing = false;
        inner.session.take()
    }

    pub fn insert_image(&self, label: &str, image: CapturedImage) {
        self.lock().images.insert(label.to_owned(), Arc::new(image));
    }

    pub fn image(&self, label: &str) -> Option<Arc<CapturedImage>> {
        self.lock().images.get(label).cloned()
    }

    pub fn remove_window(&self, label: &str) -> Option<DragFile> {
        let mut inner = self.lock();
        inner.images.remove(label);

        inner.drag_files.remove(label)
    }

    pub fn set_drag_file(&self, label: &str, file: DragFile) -> Option<DragFile> {
        self.lock().drag_files.insert(label.to_owned(), file)
    }

    pub fn drag_file(&self, label: &str) -> Option<DragFile> {
        self.lock().drag_files.get(label).cloned()
    }

    pub fn set_last_area(&self, area: PixelRect) {
        self.lock().last_area = Some(area);
    }

    pub fn last_area(&self) -> Option<PixelRect> {
        self.lock().last_area
    }
}

#[cfg(test)]
mod tests {
    use chrono::Local;

    use super::*;

    fn pending_session(id: u64) -> CaptureSession {
        let monitor = MonitorGeometry {
            bounds: PixelRect::new(0, 0, 2, 1),
            work_area: PixelRect::new(0, 0, 2, 1),
            scale_factor: 1.0,
        };

        CaptureSession {
            id,
            mode: CaptureMode::Area,
            frames: vec![MonitorFrame {
                label: "screenshot-overlay-0".to_owned(),
                monitor,
                rgba: None,
            }],
            windows: Vec::new(),
            cursor_label: "screenshot-overlay-0".to_owned(),
            previous_foreground: None,
            captured_at: Local::now(),
        }
    }

    #[test]
    fn a_session_shown_before_capture_becomes_ready_once_filled() {
        let state = ScreenshotState::default();
        state.set_session(pending_session(7));

        assert_eq!(state.with_session(CaptureSession::is_ready), Some(false));
        assert!(state.fill_session(7, vec![vec![0; 8]], vec![PixelRect::new(0, 0, 1, 1)]));
        assert_eq!(state.with_session(CaptureSession::is_ready), Some(true));
        assert_eq!(state.with_session(|session| session.windows.len()), Some(1));
    }

    #[test]
    fn pixels_for_a_cancelled_session_are_dropped() {
        let state = ScreenshotState::default();
        state.set_session(pending_session(7));
        state.take_session(Some(7));

        assert!(!state.fill_session(7, vec![vec![0; 8]], Vec::new()));
        state.set_session(pending_session(8));
        assert!(!state.fill_session(7, vec![vec![0; 8]], Vec::new()));
        assert_eq!(state.with_session(CaptureSession::is_ready), Some(false));
    }

    #[test]
    fn editor_handoff_blocks_another_capture_until_the_matching_editor_is_ready() {
        let state = ScreenshotState::default();
        state.begin_editor_handoff("screenshot-editor-1");

        assert!(!state.try_begin_capture());
        assert!(!state.complete_editor_handoff("screenshot-editor-2"));
        assert!(!state.try_begin_capture());
        assert!(state.complete_editor_handoff("screenshot-editor-1"));
        assert!(state.try_begin_capture());
    }
}
