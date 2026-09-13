import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { isMac } from "@/utils/is";

export type ShortcutPattern = string | readonly string[];

interface ShortcutKey {
  eventKey: string;
  shortcutKey: string;
}

const MODIFIER_EVENT_KEYS = ["Shift", "Control", "Alt", "Meta"] as const;

const MODIFIER_SHORTCUT_KEY: Record<string, string> = {
  Alt: "Alt",
  Control: "Control",
  Meta: "Command",
  Shift: "Shift",
};

const NAMED_SHORTCUT_KEY: Record<string, string> = {
  ArrowDown: "ArrowDown",
  ArrowLeft: "ArrowLeft",
  ArrowRight: "ArrowRight",
  ArrowUp: "ArrowUp",
  Backquote: "`",
  Backslash: "\\",
  Backspace: "Backspace",
  BracketLeft: "[",
  BracketRight: "]",
  Comma: ",",
  Delete: "Delete",
  End: "End",
  Enter: "Enter",
  Equal: "=",
  Escape: "Esc",
  Home: "Home",
  Insert: "Insert",
  Minus: "-",
  PageDown: "PageDown",
  PageUp: "PageUp",
  Period: ".",
  Quote: "'",
  Semicolon: ";",
  Slash: "/",
  Space: "Space",
  Tab: "Tab",
};

const KEY_DISPLAY: Record<string, string> = {
  Alt: isMac ? "⌥" : "Alt",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  ArrowUp: "↑",
  Backquote: "`",
  Backslash: "\\",
  Backspace: "⌫",
  Cmd: isMac ? "⌘" : "Ctrl",
  CmdOrCtrl: isMac ? "⌘" : "Ctrl",
  Comma: ",",
  Command: isMac ? "⌘" : "Win",
  CommandOrControl: isMac ? "⌘" : "Ctrl",
  Control: isMac ? "⌃" : "Ctrl",
  Ctrl: isMac ? "⌃" : "Ctrl",
  Delete: isMac ? "⌦" : "Del",
  Down: "↓",
  Enter: isMac ? "⏎" : "Enter",
  Equal: "=",
  Esc: isMac ? "⎋" : "Esc",
  Escape: isMac ? "⎋" : "Esc",
  Left: "←",
  Meta: isMac ? "⌘" : "Win",
  Minus: "-",
  Mod: isMac ? "⌘" : "Ctrl",
  Option: isMac ? "⌥" : "Alt",
  Period: ".",
  Quote: "'",
  Return: isMac ? "⏎" : "Enter",
  Right: "→",
  Semicolon: ";",
  Shift: isMac ? "⇧" : "Shift",
  Slash: "/",
  Space: isMac ? "␣" : "Space",
  Super: isMac ? "⌘" : "Win",
  Tab: isMac ? "⇥" : "Tab",
  Up: "↑",
};
/** Convert one shortcut key to the platform-specific display label. */
export const getShortcutKeyDisplay = (key: string) => {
  const normalizedKey = key.trim();
  const display = KEY_DISPLAY[normalizedKey];

  if (display) return display;

  if (/^Key[A-Z]$/.test(normalizedKey)) {
    return normalizedKey.slice(3);
  }

  if (/^Digit[0-9]$/.test(normalizedKey)) {
    return normalizedKey.slice(5);
  }

  if (normalizedKey.length === 1) {
    return normalizedKey.toUpperCase();
  }

  return normalizedKey;
};
/** Split a shortcut into displayable key badges. */
export const getShortcutKeyDisplays = (shortcut: ShortcutPattern) => {
  const keys: readonly string[] =
    typeof shortcut === "string" ? shortcut.split("+") : shortcut;

  return keys.map((key) => {
    return getShortcutKeyDisplay(key);
  });
};
/** Format a shortcut as one platform-specific display string. */
export const formatShortcutDisplay = (
  shortcut: ShortcutPattern,
  separator = " + ",
) => {
  return getShortcutKeyDisplays(shortcut).join(separator);
};
/** Normalize a shortcut literal for case- and whitespace-insensitive conflict checks. */
export const normalizeShortcutValue = (value: string) => {
  return value
    .split("+")
    .map((key) => {
      return key.trim().toLowerCase();
    })
    .filter((key) => {
      return key !== "";
    })
    .join("+");
};
/** Whether an event key is a standalone modifier key. */
export const isShortcutModifierEventKey = (eventKey: string) => {
  return MODIFIER_EVENT_KEYS.some((modifierKey) => {
    return modifierKey === eventKey;
  });
};
/** Map a keyboard event to a Tauri global-shortcut key fragment. */
export const resolveShortcutEventKey = (
  event: KeyboardEvent | ReactKeyboardEvent,
) => {
  const { code, key } = event;

  if (isShortcutModifierEventKey(key)) {
    return {
      eventKey: key,
      shortcutKey: MODIFIER_SHORTCUT_KEY[key],
    };
  }

  if (/^Key[A-Z]$/.test(code)) {
    return {
      eventKey: code,
      shortcutKey: code.slice(3),
    };
  }

  if (/^Digit[0-9]$/.test(code)) {
    return {
      eventKey: code,
      shortcutKey: code.slice(5),
    };
  }

  if (isShortcutFunctionKey(key)) {
    return {
      eventKey: key,
      shortcutKey: key,
    };
  }

  const namedKey = NAMED_SHORTCUT_KEY[code] ?? NAMED_SHORTCUT_KEY[key];
  if (!namedKey) return null;

  return {
    eventKey: code,
    shortcutKey: namedKey,
  };
};
/** Build a complete shortcut from a keyboard event in stable platform order. */
export const buildShortcutFromEvent = (
  event: KeyboardEvent | ReactKeyboardEvent,
) => {
  const primaryKey = resolveShortcutEventKey(event);

  if (!primaryKey) return null;

  if (isShortcutModifierEventKey(primaryKey.eventKey)) {
    return null;
  }

  const modifiers = resolvePressedShortcutModifiers(event, primaryKey);
  const shortcutKeys = [...modifiers, primaryKey.shortcutKey];

  if (isShortcutFunctionKey(primaryKey.shortcutKey)) {
    return shortcutKeys.join("+");
  }

  if (modifiers.length === 0) return null;

  return shortcutKeys.join("+");
};
/** Build the in-progress shortcut preview, including modifier-only input. */
export const buildShortcutPreviewFromEvent = (
  event: KeyboardEvent | ReactKeyboardEvent,
) => {
  const primaryKey = resolveShortcutEventKey(event);

  if (!primaryKey) return null;

  const modifiers = resolvePressedShortcutModifiers(event, primaryKey);

  if (isShortcutModifierEventKey(primaryKey.eventKey)) {
    return [...modifiers, primaryKey.shortcutKey].join("+");
  }

  return [...modifiers, primaryKey.shortcutKey].join("+");
};
/** Format a recorded shortcut using the platform's conventions. */
export const formatRecordedShortcut = (value: string) => {
  if (!value) return "";

  return getShortcutKeyDisplays(value).join(isMac ? "" : "+");
};
/** Whether a recorded value meets the minimum global-shortcut requirements. */
export const isRecordableShortcut = (value: string) => {
  if (!value) return false;

  const parts = value.split("+");
  const primaryKey = parts[parts.length - 1];

  if (!primaryKey) return false;

  if (isShortcutFunctionKey(primaryKey)) {
    return true;
  }

  return parts.length > 1;
};
/** Resolve pressed modifiers without duplicating the primary key. */
const resolvePressedShortcutModifiers = (
  event: KeyboardEvent | ReactKeyboardEvent,
  primaryKey: ShortcutKey,
) => {
  const modifiers: string[] = [];

  pushShortcutModifier(modifiers, event.metaKey, "Command", primaryKey);
  pushShortcutModifier(modifiers, event.ctrlKey, "Control", primaryKey);
  pushShortcutModifier(modifiers, event.altKey, "Alt", primaryKey);
  pushShortcutModifier(modifiers, event.shiftKey, "Shift", primaryKey);

  return modifiers;
};
/** Add a modifier only when it participates in a combination. */
const pushShortcutModifier = (
  modifiers: string[],
  pressed: boolean,
  shortcutKey: string,
  primaryKey: ShortcutKey,
) => {
  if (!pressed) return;
  if (primaryKey.shortcutKey === shortcutKey) return;

  modifiers.push(shortcutKey);
};
/** Whether the primary key is a function key that can be recorded alone. */
const isShortcutFunctionKey = (key: string) => {
  return /^F([1-9]|1[0-2])$/.test(key);
};
