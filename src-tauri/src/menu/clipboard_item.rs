//! Clipboard item context menu. macOS uses native muda; Windows uses a custom
//! non-focusable WebView so the previously focused application keeps focus.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::core::Result;
use crate::db::DatabaseState;
use crate::settings::Language;

#[cfg(target_os = "macos")]
pub const CLIPBOARD_MENU_ACTION_EVENT: &str = "clipboard://menu-action";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardMenuAction {
    Paste,
    PasteAsPlainText,
    PasteAsPath,
    Copy,
    SaveImage,
    OpenLink,
    SendEmail,
    RevealInFinder,
    RevealInExplorer,
    ToggleFavorite,
    TogglePinned,
    MoveToGroup,
    EditNote,
    Delete,
}

impl ClipboardMenuAction {
    pub(super) fn label(
        self,
        lang: Language,
        is_favorite: bool,
        is_pinned: bool,
        has_note: bool,
    ) -> &'static str {
        use crate::i18n::clipboard_menu::Key;

        let key = match self {
            Self::Paste => Key::Paste,
            Self::PasteAsPlainText => Key::PasteAsPlainText,
            Self::PasteAsPath => Key::PasteAsPath,
            Self::Copy => Key::Copy,
            Self::SaveImage => Key::SaveImage,
            Self::OpenLink => Key::OpenLink,
            Self::SendEmail => Key::SendEmail,
            Self::RevealInFinder => Key::RevealInFinder,
            Self::RevealInExplorer => Key::RevealInExplorer,
            Self::ToggleFavorite => {
                if is_favorite {
                    Key::Unfavorite
                } else {
                    Key::Favorite
                }
            }
            Self::TogglePinned => {
                if is_pinned {
                    Key::UnpinItem
                } else {
                    Key::PinItem
                }
            }
            Self::MoveToGroup => Key::MoveToGroup,
            Self::EditNote => {
                if has_note {
                    Key::EditNote
                } else {
                    Key::AddNote
                }
            }
            Self::Delete => Key::Delete,
        };

        crate::i18n::clipboard_menu::label(lang, key)
    }

    pub(super) fn accelerator(self) -> Option<&'static str> {
        match self {
            Self::Paste => Some("Enter"),
            Self::PasteAsPlainText | Self::PasteAsPath => Some("CmdOrCtrl+Enter"),
            Self::Copy => Some("CmdOrCtrl+C"),
            Self::SaveImage => None,
            Self::OpenLink | Self::SendEmail | Self::RevealInFinder | Self::RevealInExplorer => {
                Some("CmdOrCtrl+O")
            }
            Self::ToggleFavorite => Some("CmdOrCtrl+D"),
            Self::TogglePinned => Some("CmdOrCtrl+T"),
            Self::MoveToGroup => None,
            Self::EditNote => Some("CmdOrCtrl+M"),
            Self::Delete => Some("CmdOrCtrl+Backspace"),
        }
    }
}

pub(super) const ACTION_GROUPS: &[&[ClipboardMenuAction]] = &[
    &[
        ClipboardMenuAction::Paste,
        ClipboardMenuAction::PasteAsPlainText,
        ClipboardMenuAction::PasteAsPath,
        ClipboardMenuAction::Copy,
        ClipboardMenuAction::SaveImage,
    ],
    &[
        ClipboardMenuAction::OpenLink,
        ClipboardMenuAction::SendEmail,
        ClipboardMenuAction::RevealInFinder,
        ClipboardMenuAction::RevealInExplorer,
    ],
    &[
        ClipboardMenuAction::ToggleFavorite,
        ClipboardMenuAction::TogglePinned,
        ClipboardMenuAction::MoveToGroup,
        ClipboardMenuAction::EditNote,
    ],
    &[ClipboardMenuAction::Delete],
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ClipboardMenuGroup {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupClipboardItemMenuInput {
    pub item_id: String,
    pub available_actions: Vec<ClipboardMenuAction>,
    pub current_group_id: Option<String>,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub has_note: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ClipboardItemMenuRequest {
    pub item_id: String,
    pub available_actions: Vec<ClipboardMenuAction>,
    pub groups: Vec<ClipboardMenuGroup>,
    pub current_group_id: Option<String>,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub has_note: bool,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MenuActionPayload {
    pub action: ClipboardMenuAction,
    pub item_id: String,
    pub group_id: Option<String>,
}

// ============================================================================

// ============================================================================

#[cfg(target_os = "macos")]
mod native {
    use std::collections::HashSet;
    use std::sync::Mutex;

    use anyhow::Context;
    use tauri::menu::{
        CheckMenuItem, IsMenuItem, Menu, MenuBuilder, MenuItem, PredefinedMenuItem, Submenu,
    };
    use tauri::{AppHandle, Emitter, Manager, Wry};

    use crate::core::{AppError, Result};
    use crate::settings::Language;
    use crate::window::CLIPBOARD_WINDOW_LABEL;

    use super::{
        ClipboardItemMenuRequest, ClipboardMenuAction, ClipboardMenuGroup, MenuActionPayload,
        ACTION_GROUPS, CLIPBOARD_MENU_ACTION_EVENT,
    };

    const MENU_PREFIX: &str = "cim::";
    const MOVE_GROUP_PREFIX: &str = "cim::moveToGroup::";

    impl ClipboardMenuAction {
        fn id(self) -> &'static str {
            match self {
                Self::Paste => "cim::paste",
                Self::PasteAsPlainText => "cim::pasteAsPlainText",
                Self::PasteAsPath => "cim::pasteAsPath",
                Self::Copy => "cim::copy",
                Self::SaveImage => "cim::saveImage",
                Self::OpenLink => "cim::openLink",
                Self::SendEmail => "cim::sendEmail",
                Self::RevealInFinder => "cim::revealInFinder",
                Self::RevealInExplorer => "cim::revealInExplorer",
                Self::ToggleFavorite => "cim::toggleFavorite",
                Self::TogglePinned => "cim::togglePinned",
                Self::MoveToGroup => "cim::moveToGroup",
                Self::EditNote => "cim::editNote",
                Self::Delete => "cim::delete",
            }
        }

        fn from_id(id: &str) -> Option<Self> {
            const ALL: &[ClipboardMenuAction] = &[
                ClipboardMenuAction::Paste,
                ClipboardMenuAction::PasteAsPlainText,
                ClipboardMenuAction::PasteAsPath,
                ClipboardMenuAction::Copy,
                ClipboardMenuAction::SaveImage,
                ClipboardMenuAction::OpenLink,
                ClipboardMenuAction::SendEmail,
                ClipboardMenuAction::RevealInFinder,
                ClipboardMenuAction::RevealInExplorer,
                ClipboardMenuAction::ToggleFavorite,
                ClipboardMenuAction::TogglePinned,
                ClipboardMenuAction::MoveToGroup,
                ClipboardMenuAction::EditNote,
                ClipboardMenuAction::Delete,
            ];
            if id.starts_with(MOVE_GROUP_PREFIX) {
                return Some(ClipboardMenuAction::MoveToGroup);
            }

            ALL.iter().copied().find(|a| a.id() == id)
        }
    }

    fn move_group_id(group_id: &str) -> String {
        format!("{MOVE_GROUP_PREFIX}{group_id}")
    }

    fn group_id_from_menu_id(menu_id: &str) -> Option<String> {
        menu_id.strip_prefix(MOVE_GROUP_PREFIX).map(str::to_owned)
    }

    #[derive(Default)]
    pub(super) struct ClipboardItemMenuState {
        current: Mutex<Option<Menu<Wry>>>,
        target_item_id: Mutex<Option<String>>,
    }

    pub(super) fn init(app: &AppHandle) {
        app.manage(ClipboardItemMenuState::default());
    }

    pub(super) fn popup(app: &AppHandle, request: ClipboardItemMenuRequest) -> Result<()> {
        let state = app.try_state::<ClipboardItemMenuState>().ok_or_else(|| {
            AppError::Other(anyhow::anyhow!("ClipboardItemMenuState not managed"))
        })?;
        *state.target_item_id.lock().unwrap() = Some(request.item_id.clone());

        let window = app
            .get_webview_window(CLIPBOARD_WINDOW_LABEL)
            .ok_or_else(|| AppError::Other(anyhow::anyhow!("clipboard window missing")))?;

        let app_for_main = app.clone();
        let window_for_main = window.clone();
        let lang = crate::i18n::current_language(app);
        window
            .run_on_main_thread(move || {
                let menu = match build_menu(&app_for_main, &request, lang) {
                    Ok(m) => m,
                    Err(err) => {
                        log::warn!("build clipboard item menu failed: {err}");
                        return;
                    }
                };

                let state = app_for_main.state::<ClipboardItemMenuState>();
                {
                    let mut current = state.current.lock().unwrap();
                    *current = Some(menu);
                }

                let guard = state.current.lock().unwrap();
                let menu_ref = guard
                    .as_ref()
                    .expect("menu just stored above and lock is held");
                if let Err(err) = window_for_main.popup_menu(menu_ref) {
                    log::warn!("popup clipboard item menu failed: {err}");
                }
            })
            .map_err(|err| {
                AppError::Other(anyhow::anyhow!(
                    "schedule popup_clipboard_item_menu on main thread failed: {err}"
                ))
            })
    }

    fn build_menu(
        app: &AppHandle,
        request: &ClipboardItemMenuRequest,
        lang: Language,
    ) -> Result<Menu<Wry>> {
        let mut active: HashSet<ClipboardMenuAction> =
            request.available_actions.iter().copied().collect();
        if !request.groups.is_empty() {
            active.insert(ClipboardMenuAction::MoveToGroup);
        }

        enum Entry {
            Item(MenuItem<Wry>),
            Submenu(Submenu<Wry>),
            Sep(PredefinedMenuItem<Wry>),
        }
        let mut entries: Vec<Entry> = Vec::new();
        let mut first_group = true;

        for group in ACTION_GROUPS {
            let group_items: Vec<ClipboardMenuAction> = group
                .iter()
                .copied()
                .filter(|a| active.contains(a))
                .collect();
            if group_items.is_empty() {
                continue;
            }

            if !first_group {
                let sep = PredefinedMenuItem::separator(app).context("build separator")?;
                entries.push(Entry::Sep(sep));
            }
            first_group = false;

            for action in group_items {
                if action == ClipboardMenuAction::MoveToGroup {
                    let submenu = build_group_submenu(
                        app,
                        &request.groups,
                        request.current_group_id.as_deref(),
                        action.label(
                            lang,
                            request.is_favorite,
                            request.is_pinned,
                            request.has_note,
                        ),
                    )?;
                    entries.push(Entry::Submenu(submenu));
                    continue;
                }

                let item = MenuItem::with_id(
                    app,
                    action.id(),
                    action.label(
                        lang,
                        request.is_favorite,
                        request.is_pinned,
                        request.has_note,
                    ),
                    true,
                    action.accelerator(),
                )
                .with_context(|| format!("build menu item {}", action.id()))?;
                entries.push(Entry::Item(item));
            }
        }

        let refs: Vec<&dyn IsMenuItem<Wry>> = entries
            .iter()
            .map(|e| match e {
                Entry::Item(i) => i as &dyn IsMenuItem<Wry>,
                Entry::Submenu(s) => s as &dyn IsMenuItem<Wry>,
                Entry::Sep(s) => s as &dyn IsMenuItem<Wry>,
            })
            .collect();

        let mut builder = MenuBuilder::new(app);
        for r in refs {
            builder = builder.item(r);
        }
        builder
            .build()
            .context("build clipboard item menu")
            .map_err(Into::into)
    }

    fn build_group_submenu(
        app: &AppHandle,
        groups: &[ClipboardMenuGroup],
        current_group_id: Option<&str>,
        label: &str,
    ) -> Result<Submenu<Wry>> {
        let items = groups
            .iter()
            .map(|group| {
                let checked = current_group_id == Some(group.id.as_str());

                CheckMenuItem::with_id(
                    app,
                    move_group_id(&group.id),
                    group.name.as_str(),
                    true,
                    checked,
                    None::<&str>,
                )
                .with_context(|| format!("build move group menu item {}", group.id))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let refs: Vec<&dyn IsMenuItem<Wry>> = items
            .iter()
            .map(|item| item as &dyn IsMenuItem<Wry>)
            .collect();

        Submenu::with_items(app, label, true, &refs)
            .context("build move group submenu")
            .map_err(Into::into)
    }

    pub(super) fn handle_event(app: &AppHandle, menu_id: &str) {
        if !menu_id.starts_with(MENU_PREFIX) {
            return;
        }
        let Some(action) = ClipboardMenuAction::from_id(menu_id) else {
            log::warn!("unknown clipboard menu id: {menu_id}");
            return;
        };

        let Some(state) = app.try_state::<ClipboardItemMenuState>() else {
            log::warn!("ClipboardItemMenuState missing on menu event");
            return;
        };

        let item_id = state.target_item_id.lock().unwrap().clone();
        let Some(item_id) = item_id else {
            log::warn!("no target item id recorded for menu action {menu_id}");
            return;
        };

        let payload = MenuActionPayload {
            action,
            item_id,
            group_id: group_id_from_menu_id(menu_id),
        };
        let Some(main) = app.get_webview_window(CLIPBOARD_WINDOW_LABEL) else {
            log::warn!("clipboard window missing on clipboard menu dispatch");
            return;
        };
        if let Err(err) = main.emit(CLIPBOARD_MENU_ACTION_EVENT, payload) {
            log::warn!("emit {CLIPBOARD_MENU_ACTION_EVENT} failed: {err}");
        }
    }
}

// ============================================================================

// ============================================================================

#[allow(unused_variables)]
pub fn init(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    native::init(app);
}

#[tauri::command]
pub async fn popup_clipboard_item_menu(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    input: PopupClipboardItemMenuInput,
) -> Result<()> {
    let pool = db.pool().await;
    let groups = crate::db::groups::list_groups(&pool)
        .await?
        .into_iter()
        .filter(|group| !group.is_hidden)
        .map(|group| ClipboardMenuGroup {
            id: group.id,
            name: group.name,
        })
        .collect::<Vec<_>>();
    let request = ClipboardItemMenuRequest {
        item_id: input.item_id,
        available_actions: input.available_actions,
        groups,
        current_group_id: input.current_group_id,
        is_favorite: input.is_favorite,
        is_pinned: input.is_pinned,
        has_note: input.has_note,
    };

    #[cfg(target_os = "macos")]
    {
        native::popup(&app, request)
    }

    #[cfg(target_os = "windows")]
    {
        super::context_window::show_for_clipboard_item(&app, &request)
    }
}

#[allow(unused_variables)]
pub fn handle_menu_event(app: &AppHandle, menu_id: &str) {
    #[cfg(target_os = "macos")]
    native::handle_event(app, menu_id);
}
