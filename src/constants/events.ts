export const TAURI_EVENT = {
  BACKUP_RECEIVED: "backup://received",
  CLIPBOARD_GROUPS_UPDATED: "clipboard-groups://updated",
  CLIPBOARD_MENU_ACTION: "clipboard://menu-action",
  CLIPBOARD_UPDATED: "clipboard://updated",
  CONTEXT_MENU_SHOW: "context-menu://show",
  CONTEXT_SUBMENU_SHOW: "context-submenu://show",
  KEYBOARD_NAV: "keyboard://nav",
  PREFERENCE_HIGHLIGHT_SETTING: "preference://highlight-setting",
  PREVIEW_UPDATED: "preview://updated",
  SCREENSHOT_EDITOR_CLOSE_REQUESTED: "screenshot://editor-close-requested",
  SCREENSHOT_OVERLAY_SESSION: "screenshot://overlay-session",
  SETTINGS_UPDATED: "settings://updated",
  UPDATE_PROGRESS: "update://progress",
  WINDOW_BEFORE_DESTROY: "window://before-destroy",
  WINDOW_LIFECYCLE: "window://lifecycle",
  WINDOW_VISIBILITY: "window://visibility",
} as const;

export type TauriEvent = (typeof TAURI_EVENT)[keyof typeof TAURI_EVENT];
