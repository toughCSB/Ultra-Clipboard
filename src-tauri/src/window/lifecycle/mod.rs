//! Central window lifecycle manager. It tracks phase and generation state,
//! broadcasts lifecycle events, and coordinates idle WebView destruction.

mod descriptor;

pub use descriptor::{descriptor_for, descriptors, RetainPolicy};

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::settings::SettingsStore;

/// Default idle time before a non-clipboard WebView is destroyed.
pub const DEFAULT_IDLE_DESTROY_SECS: u64 = 60;

const MIN_IDLE_DESTROY_SECS: u64 = 5;

const MAX_IDLE_DESTROY_SECS: u64 = 24 * 60 * 60;

const CLIPBOARD_DORMANT_SECS: u64 = 5;

const BEFORE_DESTROY_DEADLINE_MS: u64 = 500;

const DEFAULT_KEEPALIVE_TIMEOUT_MS: u64 = 30_000;

const MAX_KEEPALIVE_TIMEOUT_MS: u64 = 10 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LifecyclePhase {
    /// Declared but no WebView has been created.
    NotCreated,

    /// Registered, but no transition has been observed yet.
    Created,

    /// Frontend initialization completed.
    Ready,

    /// Window is visible.
    Visible,

    HiddenWarm,

    /// Clipboard window is retained while nonessential work pauses.
    Dormant,

    /// Waiting for the final dirty/keepalive check before destruction.
    DestroyPending,

    /// WebView was destroyed and will be rebuilt on demand.
    Destroyed,
}

struct RuntimeState {
    phase: LifecyclePhase,

    /// Incremented when a hidden window becomes visible; stale timers capture this value.
    generation: u64,
    hidden_at: Option<Instant>,
    last_active_at: Instant,
    dirty_owners: HashSet<String>,
    keepalive_leases: HashMap<String, KeepaliveLease>,
}

struct KeepaliveLease {
    /// Expiration is a safety fallback, not a completion signal.
    reason: String,
    expires_at: Instant,
}

enum DestroyCheck {
    Proceed,
    Protected,
    Stale,
}

impl RuntimeState {
    fn new() -> Self {
        let now = Instant::now();

        Self {
            dirty_owners: HashSet::new(),
            generation: 0,
            hidden_at: None,
            keepalive_leases: HashMap::new(),
            last_active_at: now,
            phase: LifecyclePhase::Created,
        }
    }

    fn transition_phase(&mut self, phase: LifecyclePhase, now: Instant) {
        let previous = self.phase;

        if matches!(phase, LifecyclePhase::Visible) && previous != LifecyclePhase::Visible {
            self.generation += 1;
            self.hidden_at = None;
            self.last_active_at = now;
        }

        if matches!(phase, LifecyclePhase::HiddenWarm) && previous != LifecyclePhase::HiddenWarm {
            self.hidden_at = Some(now);
            self.last_active_at = now;
        }

        if matches!(phase, LifecyclePhase::Destroyed) {
            self.hidden_at = None;
            self.dirty_owners.clear();
            self.keepalive_leases.clear();
        }

        self.phase = phase;
    }

    fn purge_expired_leases(&mut self) -> usize {
        let now = Instant::now();
        self.keepalive_leases.retain(|owner, lease| {
            let active = lease.expires_at > now;
            if !active {
                log::warn!(
                    "window keepalive expired: owner={owner}, reason={}",
                    lease.reason
                );
            }

            active
        });

        self.keepalive_leases.len()
    }

    fn destroy_blocked(&mut self) -> bool {
        self.purge_expired_leases();

        !self.dirty_owners.is_empty() || !self.keepalive_leases.is_empty()
    }
}

const WINDOW_LIFECYCLE_EVENT: &str = "window://lifecycle";
const WINDOW_BEFORE_DESTROY_EVENT: &str = "window://before-destroy";

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LifecyclePayload<'a> {
    label: &'a str,
    phase: LifecyclePhase,
    generation: u64,
    reason: &'a str,
    visible: bool,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BeforeDestroyPayload<'a> {
    label: &'a str,
    generation: u64,
    deadline_ms: u64,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleSnapshot {
    pub label: String,
    pub phase: LifecyclePhase,
    pub generation: u64,
    pub visible: bool,
    pub retain_policy: &'static str,
    pub dirty_owner_count: usize,
    pub keepalive_count: usize,
    pub hidden_for_ms: Option<u128>,
    pub last_active_ago_ms: u128,
}

pub struct WindowLifecycleManager {
    states: Mutex<HashMap<String, RuntimeState>>,
}

impl WindowLifecycleManager {
    pub fn new() -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
        }
    }

    fn with_states<R>(&self, f: impl FnOnce(&mut HashMap<String, RuntimeState>) -> R) -> R {
        let mut guard = self.states.lock().unwrap_or_else(|poisoned| {
            log::error!("window lifecycle mutex poisoned, recovering");
            poisoned.into_inner()
        });
        f(&mut guard)
    }

    fn transition(&self, app: &AppHandle, label: &str, phase: LifecyclePhase, reason: &str) {
        let Some(descriptor) = descriptor_for(label) else {
            return;
        };

        let (previous, generation) = self.with_states(|states| {
            let entry = states
                .entry(label.to_owned())
                .or_insert_with(RuntimeState::new);

            let previous = entry.phase;
            let now = Instant::now();

            entry.transition_phase(phase, now);

            (previous, entry.generation)
        });

        log::debug!(
            "window lifecycle: {label} {previous:?} -> {phase:?} (gen {generation}, reason {reason})"
        );

        if matches!(phase, LifecyclePhase::HiddenWarm)
            && previous != LifecyclePhase::HiddenWarm
            && descriptor.retain_policy == RetainPolicy::DestroyWhenIdle
            && lightweight_mode_enabled(app)
        {
            schedule_idle_destroy(app, label, generation, idle_destroy_secs(app));
        }

        if matches!(phase, LifecyclePhase::HiddenWarm)
            && previous != LifecyclePhase::HiddenWarm
            && label == super::CLIPBOARD_WINDOW_LABEL
            && lightweight_mode_enabled(app)
        {
            schedule_clipboard_dormant(app, label, generation);
        }

        if !descriptor.emits_lifecycle {
            return;
        }

        let visible = matches!(phase, LifecyclePhase::Visible);
        if let Err(err) = app.emit(
            WINDOW_LIFECYCLE_EVENT,
            LifecyclePayload {
                label,
                phase,
                generation,
                reason,
                visible,
            },
        ) {
            log::error!("emit window lifecycle failed for {label}: {err:?}");
        }
    }
}

impl Default for WindowLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

fn manager(app: &AppHandle) -> Option<tauri::State<'_, WindowLifecycleManager>> {
    app.try_state::<WindowLifecycleManager>()
}

pub fn on_shown(app: &AppHandle, label: &str) {
    if let Some(manager) = manager(app) {
        manager.transition(app, label, LifecyclePhase::Visible, "show");
    }
}

pub fn on_hidden(app: &AppHandle, label: &str, reason: &str) {
    if let Some(manager) = manager(app) {
        manager.transition(app, label, LifecyclePhase::HiddenWarm, reason);
    }
}

pub fn on_ready(app: &AppHandle, label: &str) {
    if descriptor_for(label).is_none() {
        return;
    }

    if let Some(manager) = manager(app) {
        let should_transition = manager.with_states(|states| {
            let entry = states
                .entry(label.to_owned())
                .or_insert_with(RuntimeState::new);

            !matches!(
                entry.phase,
                LifecyclePhase::Visible
                    | LifecyclePhase::HiddenWarm
                    | LifecyclePhase::Dormant
                    | LifecyclePhase::DestroyPending
            )
        });

        if should_transition {
            manager.transition(app, label, LifecyclePhase::Ready, "ready");
        } else {
            log::debug!("window ready acknowledged without phase change: {label}");
        }
    }
}

pub fn set_dirty(app: &AppHandle, label: &str, owner: &str, dirty: bool) {
    if descriptor_for(label).is_none() {
        return;
    }

    if let Some(manager) = manager(app) {
        manager.with_states(|states| {
            let entry = states
                .entry(label.to_owned())
                .or_insert_with(RuntimeState::new);

            if dirty {
                entry.dirty_owners.insert(owner.to_owned());
            } else {
                entry.dirty_owners.remove(owner);
            }

            log::debug!(
                "window dirty: {label} owner={owner} dirty={dirty} count={}",
                entry.dirty_owners.len()
            );
        });
    }
}

pub fn acquire_keepalive(
    app: &AppHandle,
    label: &str,
    owner: &str,
    reason: &str,
    timeout_ms: Option<u64>,
) {
    if descriptor_for(label).is_none() {
        return;
    }

    let timeout = timeout_ms
        .unwrap_or(DEFAULT_KEEPALIVE_TIMEOUT_MS)
        .clamp(1_000, MAX_KEEPALIVE_TIMEOUT_MS);
    let lease = KeepaliveLease {
        expires_at: Instant::now() + Duration::from_millis(timeout),
        reason: reason.to_owned(),
    };

    if let Some(manager) = manager(app) {
        manager.with_states(|states| {
            let entry = states
                .entry(label.to_owned())
                .or_insert_with(RuntimeState::new);
            entry.keepalive_leases.insert(owner.to_owned(), lease);

            log::debug!(
                "window keepalive acquired: {label} owner={owner} reason={reason} timeoutMs={timeout}"
            );
        });
    }
}

pub fn release_keepalive(app: &AppHandle, label: &str, owner: &str) {
    if descriptor_for(label).is_none() {
        return;
    }

    if let Some(manager) = manager(app) {
        manager.with_states(|states| {
            let Some(entry) = states.get_mut(label) else {
                return;
            };
            entry.keepalive_leases.remove(owner);

            log::debug!(
                "window keepalive released: {label} owner={owner} count={}",
                entry.keepalive_leases.len()
            );
        });
    }
}

pub fn snapshot(app: &AppHandle) -> Vec<LifecycleSnapshot> {
    let now = Instant::now();
    let Some(manager) = manager(app) else {
        return Vec::new();
    };

    manager.with_states(|states| {
        descriptors()
            .iter()
            .map(|descriptor| {
                let window = app.get_webview_window(descriptor.label);
                let visible = window
                    .as_ref()
                    .and_then(|window| window.is_visible().ok())
                    .unwrap_or(false);
                let Some(state) = states.get_mut(descriptor.label) else {
                    return LifecycleSnapshot {
                        dirty_owner_count: 0,
                        generation: 0,
                        hidden_for_ms: None,
                        keepalive_count: 0,
                        label: descriptor.label.to_owned(),
                        last_active_ago_ms: 0,
                        phase: if window.is_some() {
                            LifecyclePhase::Created
                        } else {
                            LifecyclePhase::NotCreated
                        },
                        retain_policy: descriptor.retain_policy.as_str(),
                        visible,
                    };
                };
                let keepalive_count = state.purge_expired_leases();

                LifecycleSnapshot {
                    dirty_owner_count: state.dirty_owners.len(),
                    generation: state.generation,
                    hidden_for_ms: state
                        .hidden_at
                        .map(|instant| now.saturating_duration_since(instant).as_millis()),
                    keepalive_count,
                    label: descriptor.label.to_owned(),
                    last_active_ago_ms: now
                        .saturating_duration_since(state.last_active_at)
                        .as_millis(),
                    phase: snapshot_phase(state, window.is_some()),
                    retain_policy: descriptor.retain_policy.as_str(),
                    visible,
                }
            })
            .collect()
    })
}

fn snapshot_phase(state: &RuntimeState, has_window: bool) -> LifecyclePhase {
    if !has_window && state.phase == LifecyclePhase::Created && state.generation == 0 {
        return LifecyclePhase::NotCreated;
    }

    state.phase
}

pub fn rebuild_fn(label: &str) -> Option<fn(&AppHandle) -> crate::core::Result<()>> {
    descriptor_for(label).and_then(|descriptor| descriptor.build)
}

fn schedule_clipboard_dormant(app: &AppHandle, label: &str, generation: u64) {
    let app = app.clone();
    let label = label.to_owned();

    thread::spawn(move || {
        thread::sleep(Duration::from_secs(CLIPBOARD_DORMANT_SECS));

        let main_app = app.clone();
        let main_label = label.clone();
        if let Err(err) = app.run_on_main_thread(move || {
            try_enter_dormant(&main_app, &main_label, generation);
        }) {
            log::warn!("main dormant main-thread dispatch failed for {label}: {err}");
        }
    });
}

fn try_enter_dormant(app: &AppHandle, label: &str, generation: u64) {
    if !lightweight_mode_enabled(app) {
        return;
    }

    let Some(manager) = manager(app) else {
        return;
    };

    let proceed = manager.with_states(|states| match states.get(label) {
        Some(state) => state.generation == generation && state.phase == LifecyclePhase::HiddenWarm,
        None => false,
    });

    if !proceed {
        return;
    }

    manager.transition(app, label, LifecyclePhase::Dormant, "idle-dormant");
}

fn schedule_idle_destroy(app: &AppHandle, label: &str, generation: u64, timeout_secs: u64) {
    let app = app.clone();
    let label = label.to_owned();

    thread::spawn(move || {
        thread::sleep(Duration::from_secs(timeout_secs));

        let main_app = app.clone();
        let main_label = label.clone();
        if let Err(err) = app.run_on_main_thread(move || {
            try_destroy_idle(&main_app, &main_label, generation);
        }) {
            log::warn!("idle destroy main-thread dispatch failed for {label}: {err}");
        }
    });
}

fn try_destroy_idle(app: &AppHandle, label: &str, generation: u64) {
    if !lightweight_mode_enabled(app) {
        return;
    }

    let Some(manager) = manager(app) else {
        return;
    };

    let check = manager.with_states(|states| match states.get_mut(label) {
        Some(state) => {
            if state.generation != generation || state.phase != LifecyclePhase::HiddenWarm {
                return DestroyCheck::Stale;
            }

            if state.destroy_blocked() {
                DestroyCheck::Protected
            } else {
                DestroyCheck::Proceed
            }
        }
        None => DestroyCheck::Stale,
    });

    match check {
        DestroyCheck::Proceed => {}
        DestroyCheck::Protected => {
            schedule_idle_destroy(app, label, generation, idle_destroy_secs(app));
            return;
        }
        DestroyCheck::Stale => return,
    }

    manager.transition(app, label, LifecyclePhase::DestroyPending, "before-destroy");
    emit_before_destroy(app, label, generation);
    schedule_destroy_after_deadline(app, label, generation);
}

fn emit_before_destroy(app: &AppHandle, label: &str, generation: u64) {
    if let Err(err) = app.emit(
        WINDOW_BEFORE_DESTROY_EVENT,
        BeforeDestroyPayload {
            deadline_ms: BEFORE_DESTROY_DEADLINE_MS,
            generation,
            label,
        },
    ) {
        log::error!("emit window before destroy failed for {label}: {err:?}");
    }
}

fn schedule_destroy_after_deadline(app: &AppHandle, label: &str, generation: u64) {
    let app = app.clone();
    let label = label.to_owned();

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(BEFORE_DESTROY_DEADLINE_MS));

        let main_app = app.clone();
        let main_label = label.clone();
        if let Err(err) = app.run_on_main_thread(move || {
            finish_destroy_idle(&main_app, &main_label, generation);
        }) {
            log::warn!("finish destroy main-thread dispatch failed for {label}: {err}");
        }
    });
}

fn finish_destroy_idle(app: &AppHandle, label: &str, generation: u64) {
    if !lightweight_mode_enabled(app) {
        return;
    }

    let Some(manager) = manager(app) else {
        return;
    };

    let check = manager.with_states(|states| match states.get_mut(label) {
        Some(state) => {
            if state.generation != generation || state.phase != LifecyclePhase::DestroyPending {
                return DestroyCheck::Stale;
            }

            if state.destroy_blocked() {
                DestroyCheck::Protected
            } else {
                DestroyCheck::Proceed
            }
        }
        None => DestroyCheck::Stale,
    });

    match check {
        DestroyCheck::Proceed => {}
        DestroyCheck::Protected => {
            manager.transition(app, label, LifecyclePhase::HiddenWarm, "destroy-protected");
            return;
        }
        DestroyCheck::Stale => return,
    }

    if let Err(err) = super::state::save_window_state(app, label) {
        log::warn!("save window state before idle destroy failed for {label}: {err}");
    }

    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;

        if let Ok(panel) = app.get_webview_panel(label) {
            let _ = panel.to_window();
        }
    }

    if let Some(window) = app.get_webview_window(label) {
        if let Err(err) = window.destroy() {
            log::error!("idle destroy window failed for {label}: {err}");
            return;
        }
    }

    manager.transition(app, label, LifecyclePhase::Destroyed, "idle-destroy");
}

fn lightweight_mode_enabled(app: &AppHandle) -> bool {
    app.try_state::<SettingsStore>()
        .map(|settings| settings.snapshot().clipboard.window.lightweight_mode)
        .unwrap_or(true)
}

fn idle_destroy_secs(app: &AppHandle) -> u64 {
    app.try_state::<SettingsStore>()
        .map(|settings| {
            u64::from(settings.snapshot().clipboard.window.idle_destroy_seconds)
                .clamp(MIN_IDLE_DESTROY_SECS, MAX_IDLE_DESTROY_SECS)
        })
        .unwrap_or(DEFAULT_IDLE_DESTROY_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_owner_blocks_destroy_until_cleared() {
        let mut state = RuntimeState::new();

        state.dirty_owners.insert("draft".to_owned());

        assert!(state.destroy_blocked());

        state.dirty_owners.clear();

        assert!(!state.destroy_blocked());
    }

    #[test]
    fn keepalive_blocks_destroy_until_released() {
        let mut state = RuntimeState::new();

        state.keepalive_leases.insert(
            "dialog".to_owned(),
            KeepaliveLease {
                expires_at: Instant::now() + Duration::from_secs(10),
                reason: "file-dialog".to_owned(),
            },
        );

        assert!(state.destroy_blocked());

        state.keepalive_leases.remove("dialog");

        assert!(!state.destroy_blocked());
    }

    #[test]
    fn expired_keepalive_no_longer_blocks_destroy() {
        let mut state = RuntimeState::new();

        state.keepalive_leases.insert(
            "dialog".to_owned(),
            KeepaliveLease {
                expires_at: Instant::now() - Duration::from_secs(1),
                reason: "file-dialog".to_owned(),
            },
        );

        assert!(!state.destroy_blocked());
        assert!(state.keepalive_leases.is_empty());
    }

    #[test]
    fn repeated_visible_transition_keeps_generation() {
        let mut state = RuntimeState::new();
        let now = Instant::now();

        state.transition_phase(LifecyclePhase::Visible, now);
        state.transition_phase(LifecyclePhase::Visible, now + Duration::from_secs(1));

        assert_eq!(state.generation, 1);

        state.transition_phase(LifecyclePhase::HiddenWarm, now + Duration::from_secs(2));
        state.transition_phase(LifecyclePhase::Visible, now + Duration::from_secs(3));

        assert_eq!(state.generation, 2);
    }

    #[test]
    fn created_state_without_webview_snapshots_as_not_created() {
        let state = RuntimeState::new();

        assert_eq!(snapshot_phase(&state, false), LifecyclePhase::NotCreated);
        assert_eq!(snapshot_phase(&state, true), LifecyclePhase::Created);
    }

    #[test]
    fn destroyed_state_without_webview_stays_destroyed() {
        let mut state = RuntimeState::new();

        state.phase = LifecyclePhase::Destroyed;

        assert_eq!(snapshot_phase(&state, false), LifecyclePhase::Destroyed);
    }
}
