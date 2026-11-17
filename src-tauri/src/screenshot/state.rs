//! In-memory screenshot state: the active capture session and the images owned
//! by editor and pin windows. Nothing here is persisted.

use std::collections::{HashMap, HashSet};
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
    pub source_title: Option<String>,
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
    pub source_title: Option<String>,
}

#[derive(Default)]
pub struct ScreenshotState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    active_capture_id: Option<u64>,
    session: Option<CaptureSession>,
    painted_overlays: HashSet<String>,
    handoff_editor: Option<(u64, String)>,
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

    /// Reserves a capture identity before native work starts.
    pub fn try_begin_capture(&self) -> Option<u64> {
        let mut inner = self.lock();
        if inner.active_capture_id.is_some() {
            return None;
        }

        inner.next_id += 1;
        inner.active_capture_id = Some(inner.next_id);
        inner.painted_overlays.clear();
        Some(inner.next_id)
    }

    pub fn finish_capture(&self, id: u64) -> bool {
        let mut inner = self.lock();
        if inner.active_capture_id != Some(id) {
            return false;
        }

        inner.active_capture_id = None;
        inner.session = None;
        inner.painted_overlays.clear();
        inner.handoff_editor = None;
        true
    }

    pub fn begin_editor_handoff(&self, id: u64, label: &str) -> bool {
        let mut inner = self.lock();
        if inner.active_capture_id != Some(id) {
            return false;
        }

        inner.handoff_editor = Some((id, label.to_owned()));
        true
    }

    /// Hides the overlays once for the matching editor before allowing a new capture.
    pub fn complete_editor_handoff(&self, label: &str, hide: impl FnOnce()) -> bool {
        let Some(id) = self.take_editor_handoff(label) else {
            return false;
        };
        hide();
        self.finish_capture(id)
    }

    fn take_editor_handoff(&self, label: &str) -> Option<u64> {
        let mut inner = self.lock();
        let (id, pending_label) = inner.handoff_editor.as_ref()?;
        if pending_label != label {
            return None;
        }

        let id = *id;
        inner.handoff_editor = None;
        Some(id)
    }

    pub fn set_session(&self, session: CaptureSession) -> bool {
        let mut inner = self.lock();
        if inner.active_capture_id != Some(session.id) {
            return false;
        }
        inner.session = Some(session);
        inner.painted_overlays.clear();
        true
    }

    /// Records frontend first paint for an overlay in the active session.
    pub fn mark_overlay_painted(&self, id: u64, label: &str) -> bool {
        let mut inner = self.lock();
        if inner.active_capture_id != Some(id)
            || !inner
                .session
                .as_ref()
                .is_some_and(|session| session.id == id && session.frame(label).is_some())
        {
            return false;
        }

        inner.painted_overlays.insert(label.to_owned());
        true
    }

    /// A visible native window still times out until its frontend has painted.
    pub fn overlay_paint_pending(&self, id: u64, label: &str) -> bool {
        let inner = self.lock();
        inner.active_capture_id == Some(id)
            && inner
                .session
                .as_ref()
                .is_some_and(|session| session.id == id && session.frame(label).is_some())
            && !inner.painted_overlays.contains(label)
    }

    pub fn with_session<R>(&self, read: impl FnOnce(&CaptureSession) -> R) -> Option<R> {
        self.lock().session.as_ref().map(read)
    }

    /// Stores the pixels and window list of a session that was shown before its
    /// capture. Returns `false` when the session was cancelled meanwhile.
    pub fn fill_session(
        &self,
        id: u64,
        pixels: Vec<Vec<u8>>,
        windows: Vec<PixelRect>,
        source_title: Option<String>,
    ) -> bool {
        let mut inner = self.lock();
        let Some(session) = inner.session.as_mut().filter(|session| session.id == id) else {
            return false;
        };

        for (frame, rgba) in session.frames.iter_mut().zip(pixels) {
            frame.rgba = Some(Arc::new(rgba));
        }
        session.windows = windows;
        if source_title.is_some() {
            session.source_title = source_title;
        }
        true
    }

    /// Takes the matching session while keeping its capture reserved until window effects finish.
    pub fn take_session(&self, id: Option<u64>) -> Option<CaptureSession> {
        let mut inner = self.lock();
        let matches = inner
            .session
            .as_ref()
            .is_some_and(|session| id.is_none_or(|id| session.id == id));
        if !matches {
            return None;
        }

        inner
            .session
            .take()
            .inspect(|_| inner.painted_overlays.clear())
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
            source_title: None,
        }
    }

    #[test]
    fn a_session_shown_before_capture_becomes_ready_once_filled() {
        let state = ScreenshotState::default();
        let id = state.try_begin_capture().unwrap();
        assert!(state.set_session(pending_session(id)));

        assert_eq!(state.with_session(CaptureSession::is_ready), Some(false));
        assert!(state.fill_session(id, vec![vec![0; 8]], vec![PixelRect::new(0, 0, 1, 1)], None));
        assert_eq!(state.with_session(CaptureSession::is_ready), Some(true));
        assert_eq!(state.with_session(|session| session.windows.len()), Some(1));
    }

    #[test]
    fn pixels_for_a_cancelled_session_are_dropped() {
        let state = ScreenshotState::default();
        let first = state.try_begin_capture().unwrap();
        assert!(state.set_session(pending_session(first)));
        state.take_session(Some(first));
        state.finish_capture(first);

        assert!(!state.fill_session(first, vec![vec![0; 8]], Vec::new(), None));
        let second = state.try_begin_capture().unwrap();
        assert!(state.set_session(pending_session(second)));
        assert!(!state.fill_session(first, vec![vec![0; 8]], Vec::new(), None));
        assert_eq!(state.with_session(CaptureSession::is_ready), Some(false));
    }

    #[test]
    fn editor_handoff_blocks_another_capture_until_the_matching_editor_is_ready() {
        let state = ScreenshotState::default();
        let id = state.try_begin_capture().unwrap();
        assert!(state.begin_editor_handoff(id, "screenshot-editor-1"));

        assert!(state.try_begin_capture().is_none());
        assert!(!state.complete_editor_handoff("screenshot-editor-2", || {}));
        assert!(state.try_begin_capture().is_none());
        assert!(state.complete_editor_handoff("screenshot-editor-1", || {
            assert!(state.try_begin_capture().is_none());
        }));
        assert!(!state.complete_editor_handoff("screenshot-editor-1", || panic!("duplicate hide")));
        assert!(state.try_begin_capture().is_some());
    }

    #[test]
    fn late_failure_from_prior_capture_cannot_end_new_capture() {
        let state = ScreenshotState::default();
        let first = state.try_begin_capture().unwrap();
        assert!(state.finish_capture(first));
        let second = state.try_begin_capture().unwrap();
        assert!(state.set_session(pending_session(second)));

        assert!(!state.finish_capture(first));
        assert!(!state.set_session(pending_session(first)));
        assert_eq!(state.with_session(|session| session.id), Some(second));
        assert!(state.try_begin_capture().is_none());
    }
}
