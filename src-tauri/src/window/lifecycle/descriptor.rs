//! Declarative window descriptors and the static registry used by lifecycle paths.
//! Each window is declared once here so show, hide, close, and rebuild share one lookup.

use tauri::AppHandle;

use super::super::{
    build_onboarding_window, build_preference_window, build_update_window, preview,
    CLIPBOARD_PREVIEW_WINDOW_LABEL, CLIPBOARD_WINDOW_LABEL, ONBOARDING_WINDOW_LABEL,
    PREFERENCE_WINDOW_LABEL, UPDATE_WINDOW_LABEL,
};
use crate::core::Result;
use crate::screenshot::OVERLAY_WINDOW_LABEL_PREFIX;

#[cfg(target_os = "windows")]
use crate::menu::context_window::{
    build_context_menu_window, build_context_submenu_window, CONTEXT_MENU_WINDOW_LABEL,
    CONTEXT_SUBMENU_WINDOW_LABEL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainPolicy {
    Permanent,

    DestroyWhenIdle,
}

impl RetainPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Permanent => "permanent",
            Self::DestroyWhenIdle => "destroyWhenIdle",
        }
    }
}

#[derive(Clone, Copy)]
pub struct WindowDescriptor {
    /// Tauri window label.
    pub label: &'static str,

    pub emits_lifecycle: bool,

    pub retain_policy: RetainPolicy,

    pub build: Option<fn(&AppHandle) -> Result<()>>,
}

static DESCRIPTORS: &[WindowDescriptor] = &[
    WindowDescriptor {
        label: CLIPBOARD_WINDOW_LABEL,
        emits_lifecycle: true,
        retain_policy: RetainPolicy::Permanent,
        build: None,
    },
    WindowDescriptor {
        label: PREFERENCE_WINDOW_LABEL,
        emits_lifecycle: true,
        retain_policy: RetainPolicy::DestroyWhenIdle,
        build: Some(build_preference_window),
    },
    WindowDescriptor {
        label: ONBOARDING_WINDOW_LABEL,
        emits_lifecycle: true,
        retain_policy: RetainPolicy::DestroyWhenIdle,
        build: Some(build_onboarding_window),
    },
    WindowDescriptor {
        label: UPDATE_WINDOW_LABEL,
        emits_lifecycle: true,
        retain_policy: RetainPolicy::DestroyWhenIdle,
        build: Some(build_update_window),
    },
    WindowDescriptor {
        label: CLIPBOARD_PREVIEW_WINDOW_LABEL,
        emits_lifecycle: true,
        retain_policy: RetainPolicy::DestroyWhenIdle,
        build: Some(preview::build_clipboard_preview_window),
    },
    #[cfg(target_os = "windows")]
    WindowDescriptor {
        label: CONTEXT_MENU_WINDOW_LABEL,
        emits_lifecycle: true,
        retain_policy: RetainPolicy::DestroyWhenIdle,
        build: Some(build_context_menu_window),
    },
    #[cfg(target_os = "windows")]
    WindowDescriptor {
        label: CONTEXT_SUBMENU_WINDOW_LABEL,
        emits_lifecycle: true,
        retain_policy: RetainPolicy::DestroyWhenIdle,
        build: Some(build_context_submenu_window),
    },
];

/// Windows created per monitor or per capture. `label` holds a prefix, and
/// matching windows are never rebuilt by the lifecycle manager.
static PREFIX_DESCRIPTORS: &[WindowDescriptor] = &[WindowDescriptor {
    label: OVERLAY_WINDOW_LABEL_PREFIX,
    emits_lifecycle: false,
    retain_policy: RetainPolicy::DestroyWhenIdle,
    build: None,
}];

pub fn descriptor_for(label: &str) -> Option<&'static WindowDescriptor> {
    DESCRIPTORS.iter().find(|d| d.label == label).or_else(|| {
        PREFIX_DESCRIPTORS
            .iter()
            .find(|d| label.starts_with(d.label))
    })
}

pub fn descriptors() -> &'static [WindowDescriptor] {
    DESCRIPTORS
}
