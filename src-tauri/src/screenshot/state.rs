//! In-memory screenshot state: the active capture session and the images owned
//! by editor and pin windows. Nothing here is persisted.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, Local};

use super::geometry::PixelRect;
use super::session::{Effect, Event, Session};
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
    /// The monitor the image was captured on, kept so a pin can be placed
    /// even when the current monitors cannot be read.
    pub monitor: MonitorGeometry,
    /// Virtual desktop position of the image's top-left pixel when it was captured.
    pub origin_x: i32,
    pub origin_y: i32,
    pub rgba: Vec<u8>,
    pub captured_at: DateTime<Local>,
    pub source_title: Option<String>,
}

#[cfg(test)]
impl MonitorGeometry {
    /// A test monitor whose work area covers its whole bounds.
    pub fn whole(bounds: PixelRect, scale_factor: f64) -> Self {
        Self {
            bounds,
            work_area: bounds,
            scale_factor,
        }
    }
}

#[cfg(test)]
impl CapturedImage {
    /// A test image without pixels covering `area` of `monitor`.
    pub fn blank(monitor: MonitorGeometry, area: PixelRect) -> Self {
        Self {
            width: area.width,
            height: area.height,
            monitor,
            origin_x: area.x,
            origin_y: area.y,
            rgba: Vec::new(),
            captured_at: Local::now(),
            source_title: None,
        }
    }
}

#[derive(Default)]
pub struct ScreenshotState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    session: Session,
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
        self.lock().session.next_id()
    }

    /// Runs one capture session transition and returns the effects it asks for.
    pub fn apply(&self, event: Event) -> Vec<Effect> {
        self.lock().session.apply(event)
    }

    /// Reserves a capture identity before native work starts.
    pub fn try_begin_capture(&self) -> Option<u64> {
        self.lock().session.reserve()
    }

    pub fn with_session<R>(&self, read: impl FnOnce(&CaptureSession) -> R) -> Option<R> {
        self.lock().session.current().map(read)
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
    use super::*;

    #[test]
    fn reservations_go_through_the_session_rules() {
        let state = ScreenshotState::default();
        let id = state.try_begin_capture().unwrap();

        assert!(state.try_begin_capture().is_none());
        assert!(state
            .apply(Event::Fail {
                id,
                permission_missing: false
            })
            .is_empty());
        assert!(state.try_begin_capture().is_some());
    }
}
