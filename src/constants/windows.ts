export const WINDOW_LABEL = {
  CLIPBOARD: "clipboard",

  CONTEXT_MENU: "context-menu",
  /**
   * Custom context submenu window on Windows.
   */
  CONTEXT_SUBMENU: "context-submenu",

  ONBOARDING: "onboarding",

  PREFERENCE: "preference",

  PREVIEW: "clipboard-preview",

  UPDATE: "update",
} as const;

/**
 * Prefixes of windows created per capture or per monitor.
 */
export const WINDOW_LABEL_PREFIX = {
  SCREENSHOT_EDITOR: "screenshot-editor-",
  SCREENSHOT_OVERLAY: "screenshot-overlay-",
  SCREENSHOT_PIN: "screenshot-pin-",
} as const;
