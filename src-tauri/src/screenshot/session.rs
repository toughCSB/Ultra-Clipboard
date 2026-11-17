//! Capture session rules. Given the current phase and an [`Event`], the session
//! returns the effects the adapter must run. It never touches Tauri, threads or
//! windows, so every transition is testable with plain values.
//!
//! Event and effect ordering is the caller's job: apply an event and run its
//! effects on the main thread so a session check stays ordered with
//! cancellation.

use std::collections::HashSet;
use std::sync::Arc;

use super::geometry::PixelRect;
use super::state::{CaptureSession, CapturedImage, MonitorGeometry};
use super::EDITOR_WINDOW_LABEL_PREFIX;

/// When an overlay becomes visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reveal {
    /// After the frontend draws the frozen frame.
    AfterPaint,
    /// Right away and without focus, over the live screen, before the capture.
    Immediate,
}

pub enum Event {
    /// Reserves a capture identity before native work starts.
    Reserve,
    /// Publishes the session of the reserved capture, with or without pixels.
    Publish(CaptureSession),
    /// Stores the pixels of a session that was published without them.
    Fill {
        id: u64,
        pixels: Vec<Vec<u8>>,
        windows: Vec<PixelRect>,
        source_title: Option<String>,
    },
    /// The frontend painted an overlay of the session.
    OverlayPainted { id: u64, label: String },
    /// The paint timeout of an overlay elapsed.
    PaintTimeout { id: u64, label: String },
    /// The user confirmed a selection on an overlay.
    CommitSelection {
        id: u64,
        frame_label: String,
        selection: PixelRect,
    },
    /// A capture without an overlay produced its image.
    DirectCaptured {
        id: u64,
        image: CapturedImage,
        monitor: MonitorGeometry,
    },
    /// The editor of a selection could not be opened.
    EditorFailed { id: u64 },
    /// The editor with this label is ready or closed.
    EditorSettled { label: String },
    /// The user or the system cancels the session. `None` matches any session.
    Cancel { id: Option<u64> },
    /// Native capture failed for this capture identity.
    Fail { id: u64, permission_missing: bool },
}

pub enum Effect {
    Reserved(u64),
    PresentOverlays {
        id: u64,
        reveal: Reveal,
    },
    RefreshOverlays {
        id: u64,
    },
    HideOverlays,
    RestoreForeground(Option<isize>),
    /// Crops `selection` out of `frame_label` and opens the editor named `editor_label`.
    OpenEditor {
        session: CaptureSession,
        frame_label: String,
        selection: PixelRect,
        editor_label: String,
    },
    OpenDirectEditor {
        image: CapturedImage,
        monitor: MonitorGeometry,
    },
    OpenPermissionPreference,
    /// The selection was confirmed before every monitor's pixels arrived.
    RejectUnreadyCommit,
}

#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    Reserved {
        id: u64,
    },
    Presented {
        session: CaptureSession,
        painted: HashSet<String>,
    },
    Handoff {
        id: u64,
        editor_label: String,
    },
}

#[derive(Default)]
pub struct Session {
    next_id: u64,
    phase: Phase,
}

impl Session {
    /// Hands out an identity shared by captures, editors and pins.
    pub fn next_id(&mut self) -> u64 {
        self.next_id += 1;

        self.next_id
    }

    /// The session whose overlays are on screen or about to be.
    pub fn current(&self) -> Option<&CaptureSession> {
        match &self.phase {
            Phase::Presented { session, .. } => Some(session),
            _ => None,
        }
    }

    pub fn apply(&mut self, event: Event) -> Vec<Effect> {
        match event {
            Event::Reserve => self.reserve(),
            Event::Publish(session) => self.publish(session),
            Event::Fill {
                id,
                pixels,
                windows,
                source_title,
            } => self.fill(id, pixels, windows, source_title),
            Event::OverlayPainted { id, label } => self.overlay_painted(id, label),
            Event::PaintTimeout { id, label } => self.paint_timeout(id, &label),
            Event::CommitSelection {
                id,
                frame_label,
                selection,
            } => self.commit_selection(id, frame_label, selection),
            Event::DirectCaptured { id, image, monitor } => {
                self.direct_captured(id, image, monitor)
            }
            Event::EditorFailed { id } => self.editor_failed(id),
            Event::EditorSettled { label } => self.editor_settled(&label),
            Event::Cancel { id } => self.cancel(id),
            Event::Fail {
                id,
                permission_missing,
            } => self.fail(id, permission_missing),
        }
    }

    fn reserve(&mut self) -> Vec<Effect> {
        if !matches!(self.phase, Phase::Idle) {
            return Vec::new();
        }

        let id = self.next_id();
        self.phase = Phase::Reserved { id };

        vec![Effect::Reserved(id)]
    }

    fn publish(&mut self, session: CaptureSession) -> Vec<Effect> {
        let accepts = match &self.phase {
            Phase::Reserved { id } => *id == session.id,
            Phase::Presented {
                session: current, ..
            } => current.id == session.id,
            _ => false,
        };
        if !accepts {
            return Vec::new();
        }

        let id = session.id;
        let reveal = if session.is_ready() {
            Reveal::AfterPaint
        } else {
            Reveal::Immediate
        };
        self.phase = Phase::Presented {
            session,
            painted: HashSet::new(),
        };

        vec![Effect::PresentOverlays { id, reveal }]
    }

    fn fill(
        &mut self,
        id: u64,
        pixels: Vec<Vec<u8>>,
        windows: Vec<PixelRect>,
        source_title: Option<String>,
    ) -> Vec<Effect> {
        let Phase::Presented { session, .. } = &mut self.phase else {
            return Vec::new();
        };
        if session.id != id {
            return Vec::new();
        }

        for (frame, rgba) in session.frames.iter_mut().zip(pixels) {
            frame.rgba = Some(Arc::new(rgba));
        }
        session.windows = windows;
        if source_title.is_some() {
            session.source_title = source_title;
        }

        vec![Effect::RefreshOverlays { id }]
    }

    fn overlay_painted(&mut self, id: u64, label: String) -> Vec<Effect> {
        if let Phase::Presented { session, painted } = &mut self.phase {
            if session.id == id && session.frame(&label).is_some() {
                painted.insert(label);
            }
        }

        Vec::new()
    }

    /// A visible native window still times out until its frontend has painted.
    fn paint_timeout(&mut self, id: u64, label: &str) -> Vec<Effect> {
        let pending = matches!(
            &self.phase,
            Phase::Presented { session, painted }
                if session.id == id
                    && session.frame(label).is_some()
                    && !painted.contains(label)
        );
        if !pending {
            return Vec::new();
        }

        self.cancel(Some(id))
    }

    fn commit_selection(
        &mut self,
        id: u64,
        frame_label: String,
        selection: PixelRect,
    ) -> Vec<Effect> {
        let Phase::Presented { session, .. } = &self.phase else {
            return Vec::new();
        };
        if session.id != id {
            return Vec::new();
        }
        if !session.is_ready() {
            return vec![Effect::RejectUnreadyCommit];
        }

        let editor_label = format!("{EDITOR_WINDOW_LABEL_PREFIX}{}", self.next_id());
        let Phase::Presented { session, .. } = std::mem::replace(
            &mut self.phase,
            Phase::Handoff {
                id,
                editor_label: editor_label.clone(),
            },
        ) else {
            return Vec::new();
        };

        vec![Effect::OpenEditor {
            session,
            frame_label,
            selection,
            editor_label,
        }]
    }

    fn direct_captured(
        &mut self,
        id: u64,
        image: CapturedImage,
        monitor: MonitorGeometry,
    ) -> Vec<Effect> {
        if !matches!(self.phase, Phase::Reserved { id: current } if current == id) {
            return Vec::new();
        }
        self.phase = Phase::Idle;

        vec![Effect::OpenDirectEditor { image, monitor }]
    }

    fn editor_failed(&mut self, id: u64) -> Vec<Effect> {
        if !matches!(self.phase, Phase::Handoff { id: current, .. } if current == id) {
            return Vec::new();
        }
        self.phase = Phase::Idle;

        vec![Effect::HideOverlays]
    }

    /// Hides the overlays once, for the matching editor, before another capture may start.
    fn editor_settled(&mut self, label: &str) -> Vec<Effect> {
        if !matches!(&self.phase, Phase::Handoff { editor_label, .. } if editor_label == label) {
            return Vec::new();
        }
        self.phase = Phase::Idle;

        vec![Effect::HideOverlays]
    }

    fn cancel(&mut self, id: Option<u64>) -> Vec<Effect> {
        let Phase::Presented { session, .. } = &self.phase else {
            return Vec::new();
        };
        if id.is_some_and(|id| session.id != id) {
            return Vec::new();
        }

        self.end_presented()
    }

    fn fail(&mut self, id: u64, permission_missing: bool) -> Vec<Effect> {
        let mut effects = match &self.phase {
            Phase::Reserved { id: current } if *current == id => {
                self.phase = Phase::Idle;
                Vec::new()
            }
            Phase::Presented { session, .. } if session.id == id => self.end_presented(),
            _ => return Vec::new(),
        };

        if permission_missing {
            effects.push(Effect::OpenPermissionPreference);
        }

        effects
    }

    fn end_presented(&mut self) -> Vec<Effect> {
        let Phase::Presented { session, .. } = std::mem::take(&mut self.phase) else {
            return Vec::new();
        };

        vec![
            Effect::HideOverlays,
            Effect::RestoreForeground(session.previous_foreground),
        ]
    }
}

#[cfg(test)]
mod tests {
    use chrono::Local;

    use super::*;
    use crate::screenshot::state::MonitorFrame;
    use crate::screenshot::CaptureMode;

    fn kinds(effects: &[Effect]) -> Vec<&'static str> {
        effects
            .iter()
            .map(|effect| match effect {
                Effect::Reserved(_) => "reserved",
                Effect::PresentOverlays {
                    reveal: Reveal::Immediate,
                    ..
                } => "present-now",
                Effect::PresentOverlays { .. } => "present-after-paint",
                Effect::RefreshOverlays { .. } => "refresh",
                Effect::HideOverlays => "hide",
                Effect::RestoreForeground(_) => "restore",
                Effect::OpenEditor { .. } => "open-editor",
                Effect::OpenDirectEditor { .. } => "open-direct-editor",
                Effect::OpenPermissionPreference => "permission-preference",
                Effect::RejectUnreadyCommit => "reject-unready",
            })
            .collect()
    }

    fn label(index: usize) -> String {
        format!("screenshot-overlay-{index}")
    }

    fn session(id: u64, foreground: Option<isize>, ready: bool) -> CaptureSession {
        let bounds = PixelRect::new(0, 0, 2, 1);

        CaptureSession {
            id,
            mode: CaptureMode::Area,
            frames: vec![MonitorFrame {
                label: label(0),
                monitor: MonitorGeometry {
                    bounds,
                    work_area: bounds,
                    scale_factor: 1.0,
                },
                rgba: ready.then(|| Arc::new(vec![0; 8])),
            }],
            windows: Vec::new(),
            cursor_label: label(0),
            previous_foreground: foreground,
            captured_at: Local::now(),
            source_title: None,
        }
    }

    fn reserve(state: &mut Session) -> u64 {
        match state.apply(Event::Reserve).as_slice() {
            [Effect::Reserved(id)] => *id,
            _ => panic!("capture was not reserved"),
        }
    }

    fn early_session(state: &mut Session, foreground: Option<isize>) -> u64 {
        let id = reserve(state);
        let effects = state.apply(Event::Publish(session(id, foreground, false)));
        assert_eq!(kinds(&effects), ["present-now"]);

        id
    }

    fn fill(state: &mut Session, id: u64) -> Vec<Effect> {
        state.apply(Event::Fill {
            id,
            pixels: vec![vec![0; 8]],
            windows: vec![PixelRect::new(0, 0, 1, 1)],
            source_title: None,
        })
    }

    fn commit(state: &mut Session, id: u64) -> Vec<Effect> {
        state.apply(Event::CommitSelection {
            id,
            frame_label: label(0),
            selection: PixelRect::new(0, 0, 1, 1),
        })
    }

    fn cancel(state: &mut Session, id: Option<u64>) -> Vec<Effect> {
        state.apply(Event::Cancel { id })
    }

    fn fail(state: &mut Session, id: u64, permission_missing: bool) -> Vec<Effect> {
        state.apply(Event::Fail {
            id,
            permission_missing,
        })
    }

    fn editor_label(effects: &[Effect]) -> String {
        match effects {
            [Effect::OpenEditor { editor_label, .. }] => editor_label.clone(),
            _ => panic!("selection did not open an editor"),
        }
    }

    #[test]
    fn only_one_capture_is_reserved_at_a_time() {
        let mut state = Session::default();
        let first = reserve(&mut state);

        assert!(state.apply(Event::Reserve).is_empty());
        assert!(fail(&mut state, first, false).is_empty());
        assert_ne!(reserve(&mut state), first);
    }

    #[test]
    fn publishing_needs_the_reserved_identity() {
        let mut state = Session::default();
        assert!(state
            .apply(Event::Publish(session(1, None, true)))
            .is_empty());

        let id = reserve(&mut state);
        assert!(state
            .apply(Event::Publish(session(id + 1, None, true)))
            .is_empty());
        assert!(state.current().is_none());
        assert_eq!(
            kinds(&state.apply(Event::Publish(session(id, None, true)))),
            ["present-after-paint"]
        );
        assert_eq!(state.current().map(|session| session.id), Some(id));
    }

    #[test]
    fn a_session_shown_before_capture_becomes_ready_once_filled() {
        let mut state = Session::default();
        let id = early_session(&mut state, None);
        assert_eq!(state.current().map(CaptureSession::is_ready), Some(false));

        assert_eq!(kinds(&fill(&mut state, id)), ["refresh"]);

        assert_eq!(state.current().map(CaptureSession::is_ready), Some(true));
        assert_eq!(
            state.current().map(|session| session.windows.len()),
            Some(1)
        );
    }

    #[test]
    fn pixels_for_a_cancelled_session_are_dropped() {
        let mut state = Session::default();
        let first = early_session(&mut state, None);
        cancel(&mut state, Some(first));
        assert!(fill(&mut state, first).is_empty());

        let second = early_session(&mut state, None);

        assert!(fill(&mut state, first).is_empty());
        assert_eq!(state.current().map(CaptureSession::is_ready), Some(false));
        assert_eq!(state.current().map(|session| session.id), Some(second));
    }

    #[test]
    fn stale_cancel_does_not_hide_or_restore_the_current_capture() {
        let mut state = Session::default();
        let old = early_session(&mut state, Some(42));
        let effects = cancel(&mut state, Some(old));
        assert_eq!(kinds(&effects), ["hide", "restore"]);
        assert!(matches!(effects[1], Effect::RestoreForeground(Some(42))));

        let current = early_session(&mut state, Some(84));

        assert!(cancel(&mut state, Some(old)).is_empty());
        assert_eq!(state.current().map(|session| session.id), Some(current));
        let effects = cancel(&mut state, Some(current));
        assert_eq!(kinds(&effects), ["hide", "restore"]);
        assert!(matches!(effects[1], Effect::RestoreForeground(Some(84))));
        assert!(state.current().is_none());
    }

    #[test]
    fn cancel_without_an_id_ends_whichever_session_is_shown() {
        let mut state = Session::default();
        early_session(&mut state, None);

        assert_eq!(kinds(&cancel(&mut state, None)), ["hide", "restore"]);
        assert!(cancel(&mut state, None).is_empty());
    }

    #[test]
    fn cancel_does_not_end_a_reserved_capture_that_has_no_overlays() {
        let mut state = Session::default();
        reserve(&mut state);

        assert!(cancel(&mut state, None).is_empty());
        assert!(state.apply(Event::Reserve).is_empty());
    }

    #[test]
    fn stale_fail_cannot_end_a_newer_capture() {
        let mut state = Session::default();
        let old = reserve(&mut state);
        assert!(fail(&mut state, old, false).is_empty());
        let current = early_session(&mut state, None);

        assert!(fail(&mut state, old, true).is_empty());

        assert_eq!(state.current().map(|session| session.id), Some(current));
        assert!(state.apply(Event::Reserve).is_empty());
    }

    #[test]
    fn failing_during_an_early_session_hides_and_restores_once() {
        let mut state = Session::default();
        let id = early_session(&mut state, Some(7));

        let effects = fail(&mut state, id, false);

        assert_eq!(kinds(&effects), ["hide", "restore"]);
        assert!(matches!(effects[1], Effect::RestoreForeground(Some(7))));
        assert!(fail(&mut state, id, false).is_empty());
        assert!(cancel(&mut state, Some(id)).is_empty());
    }

    #[test]
    fn failing_before_overlays_exist_releases_the_reservation_without_window_effects() {
        let mut state = Session::default();
        let id = reserve(&mut state);

        assert!(fail(&mut state, id, false).is_empty());

        assert!(!state.apply(Event::Reserve).is_empty());
    }

    #[test]
    fn failing_after_overlays_were_published_still_releases_the_reservation() {
        let mut state = Session::default();
        let id = reserve(&mut state);
        state.apply(Event::Publish(session(id, None, true)));

        assert_eq!(kinds(&fail(&mut state, id, false)), ["hide", "restore"]);

        assert!(!state.apply(Event::Reserve).is_empty());
    }

    #[test]
    fn the_permission_preference_opens_only_when_the_failing_capture_was_current() {
        let mut state = Session::default();
        let id = reserve(&mut state);
        assert_eq!(
            kinds(&fail(&mut state, id, true)),
            ["permission-preference"]
        );
        assert!(fail(&mut state, id, true).is_empty());

        let early = early_session(&mut state, None);
        assert_eq!(
            kinds(&fail(&mut state, early, true)),
            ["hide", "restore", "permission-preference"]
        );

        let current = early_session(&mut state, None);
        assert!(fail(&mut state, early, true).is_empty());
        assert_eq!(state.current().map(|session| session.id), Some(current));
    }

    #[test]
    fn committing_before_pixels_arrive_is_rejected_and_keeps_the_session() {
        let mut state = Session::default();
        let id = early_session(&mut state, None);

        assert_eq!(kinds(&commit(&mut state, id)), ["reject-unready"]);

        assert_eq!(state.current().map(|session| session.id), Some(id));
        fill(&mut state, id);
        assert_eq!(kinds(&commit(&mut state, id)), ["open-editor"]);
    }

    #[test]
    fn committing_a_stale_identity_does_nothing() {
        let mut state = Session::default();
        let id = early_session(&mut state, None);
        fill(&mut state, id);

        assert!(commit(&mut state, id + 1).is_empty());
        assert!(commit(&mut Session::default(), id).is_empty());
        assert_eq!(state.current().map(|session| session.id), Some(id));
    }

    #[test]
    fn a_committed_selection_starts_a_handoff_that_blocks_another_capture() {
        let mut state = Session::default();
        let id = early_session(&mut state, None);
        fill(&mut state, id);

        let effects = commit(&mut state, id);

        assert!(editor_label(&effects).starts_with(EDITOR_WINDOW_LABEL_PREFIX));
        assert!(state.current().is_none());
        assert!(state.apply(Event::Reserve).is_empty());
        assert!(cancel(&mut state, Some(id)).is_empty());
        assert!(fail(&mut state, id, true).is_empty());
        assert!(state.apply(Event::Reserve).is_empty());
    }

    #[test]
    fn the_handoff_completes_once_for_the_matching_editor() {
        let mut state = Session::default();
        let id = early_session(&mut state, None);
        fill(&mut state, id);
        let label = editor_label(&commit(&mut state, id));

        let other = format!("{EDITOR_WINDOW_LABEL_PREFIX}{}", state.next_id());
        assert!(state
            .apply(Event::EditorSettled { label: other })
            .is_empty());
        assert!(state.apply(Event::Reserve).is_empty());

        let effects = state.apply(Event::EditorSettled {
            label: label.clone(),
        });
        assert_eq!(kinds(&effects), ["hide"]);
        assert!(state.apply(Event::EditorSettled { label }).is_empty());
        assert!(!state.apply(Event::Reserve).is_empty());
    }

    #[test]
    fn a_late_settle_from_a_previous_editor_cannot_end_the_next_handoff() {
        let mut state = Session::default();
        let first = early_session(&mut state, None);
        fill(&mut state, first);
        let first_label = editor_label(&commit(&mut state, first));
        state.apply(Event::EditorSettled {
            label: first_label.clone(),
        });
        let second = early_session(&mut state, None);
        fill(&mut state, second);
        let second_label = editor_label(&commit(&mut state, second));

        assert!(state
            .apply(Event::EditorSettled { label: first_label })
            .is_empty());
        assert!(state.apply(Event::Reserve).is_empty());
        assert_eq!(
            kinds(&state.apply(Event::EditorSettled {
                label: second_label
            })),
            ["hide"]
        );
    }

    #[test]
    fn a_failed_editor_releases_the_handoff_and_hides_the_overlays() {
        let mut state = Session::default();
        let id = early_session(&mut state, None);
        fill(&mut state, id);
        commit(&mut state, id);

        assert!(state.apply(Event::EditorFailed { id: id + 1 }).is_empty());
        assert_eq!(kinds(&state.apply(Event::EditorFailed { id })), ["hide"]);
        assert!(!state.apply(Event::Reserve).is_empty());
    }

    #[test]
    fn a_direct_capture_releases_its_reservation_and_opens_the_editor() {
        let image = || CapturedImage {
            width: 1,
            height: 1,
            scale_factor: 1.0,
            origin_x: 0,
            origin_y: 0,
            rgba: vec![0; 4],
            captured_at: Local::now(),
            source_title: None,
        };
        let monitor = session(0, None, true).frames[0].monitor;
        let mut state = Session::default();
        let id = reserve(&mut state);

        assert!(state
            .apply(Event::DirectCaptured {
                id: id + 1,
                image: image(),
                monitor,
            })
            .is_empty());
        let effects = state.apply(Event::DirectCaptured {
            id,
            image: image(),
            monitor,
        });

        assert_eq!(kinds(&effects), ["open-direct-editor"]);
        assert!(!state.apply(Event::Reserve).is_empty());
    }

    #[test]
    fn native_visibility_without_frontend_paint_still_times_out() {
        let mut state = Session::default();
        let id = early_session(&mut state, Some(3));

        let effects = state.apply(Event::PaintTimeout {
            id,
            label: label(0),
        });

        assert_eq!(kinds(&effects), ["hide", "restore"]);
        assert!(state.current().is_none());
    }

    #[test]
    fn wrong_id_or_label_cannot_acknowledge_or_time_out_a_session() {
        let mut state = Session::default();
        let id = early_session(&mut state, None);

        state.apply(Event::OverlayPainted {
            id: id + 1,
            label: label(0),
        });
        state.apply(Event::OverlayPainted {
            id,
            label: label(1),
        });
        assert!(state
            .apply(Event::PaintTimeout {
                id: id + 1,
                label: label(0)
            })
            .is_empty());
        assert!(state
            .apply(Event::PaintTimeout {
                id,
                label: label(1)
            })
            .is_empty());

        assert_eq!(
            kinds(&state.apply(Event::PaintTimeout {
                id,
                label: label(0)
            })),
            ["hide", "restore"]
        );
    }

    #[test]
    fn a_painted_overlay_prevents_its_timeout_and_an_old_timer_cannot_end_a_new_session() {
        let mut state = Session::default();
        let old = early_session(&mut state, None);
        state.apply(Event::OverlayPainted {
            id: old,
            label: label(0),
        });
        assert!(state
            .apply(Event::PaintTimeout {
                id: old,
                label: label(0)
            })
            .is_empty());
        cancel(&mut state, Some(old));

        let current = early_session(&mut state, None);

        assert!(state
            .apply(Event::PaintTimeout {
                id: old,
                label: label(0)
            })
            .is_empty());
        assert_eq!(state.current().map(|session| session.id), Some(current));
        assert_eq!(
            kinds(&state.apply(Event::PaintTimeout {
                id: current,
                label: label(0)
            })),
            ["hide", "restore"]
        );
    }

    #[test]
    fn publishing_again_forgets_which_overlays_painted() {
        let mut state = Session::default();
        let id = early_session(&mut state, None);
        state.apply(Event::OverlayPainted {
            id,
            label: label(0),
        });
        state.apply(Event::Publish(session(id, None, false)));

        assert_eq!(
            kinds(&state.apply(Event::PaintTimeout {
                id,
                label: label(0)
            })),
            ["hide", "restore"]
        );
    }
}
