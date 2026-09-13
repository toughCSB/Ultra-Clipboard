import type {
  ClipboardCategory,
  ClipboardItemSort,
  ClipboardRange,
} from "./clipboard";

/**
 * Settings contract mirroring `src-tauri/src/settings/model.rs::Settings`.
 *
 * Field names and enum literals must match Rust's `serde` output exactly.
 * Update this file whenever the Rust model changes.
 */

/** Rust enum `Theme` (`rename_all = "lowercase"). */
export type Theme = "auto" | "light" | "dark";

/** Rust enum `Language` (explicit `serde(rename)`). */
export type Language = "ko-KR" | "en-US";

export type AutoPaste =
  | "disabled"
  | "singleClickPaste"
  | "doubleClickPaste"
  | "singleClickCopy"
  | "doubleClickCopy";

export type MiddleClickAction =
  | "disabled"
  | "singleClickPaste"
  | "singleClickPastePlain"
  | "singleClickCopy"
  | "singleClickCopyPlain";

export type ItemAction =
  | "paste"
  | "pastePlain"
  | "pastePath"
  | "copy"
  | "copyPlain"
  | "openLink"
  | "sendEmail"
  | "reveal"
  | "note"
  | "star"
  | "pinItem"
  | "delete";

export type CaptureKind = "files" | "image" | "html" | "rtf" | "text";

export type RetentionUnit = "hours" | "days" | "weeks" | "months" | "forever";

export type WindowPosition = "remember" | "followCursor" | "center";

export type WindowOpenRangeSelection = "preserve" | ClipboardRange;

export type WindowOpenCategorySelection =
  | "preserve"
  | "all"
  | ClipboardCategory;

export type WindowOpenGroupSelection = "preserve" | "all" | `group:${string}`;

export type PreviewHoverDelayMs = "ms300" | "ms500" | "ms1000";

export type UpdateFrequency = "daily" | "weekly" | "monthly";

export type OnboardingLegacyImportType = "normal" | "favorite";

export interface General {
  autoStart: boolean;
  runAsAdmin: boolean;
  trayIcon: boolean;
  dockIcon: boolean;
}

export interface Appearance {
  theme: Theme;
  language: Language;
}

export interface Shortcuts {
  openClipboard: string;
  openPreference: string;
  winV: boolean;
}

export interface Content {
  autoPaste: AutoPaste;
  middleClick: MiddleClickAction;
  copyPlain: boolean;
  copyThenHideWindow: boolean;
  pastePlain: boolean;
  pasteFilesAsPath: boolean;
  showOriginalPreview: boolean;
  deleteConfirm: boolean;
  deleteFavoriteItems: boolean;
  deleteFavoriteConfirm: boolean;
  deletePinnedItems: boolean;
  deletePinnedConfirm: boolean;
  deleteFavoriteItemsOnlyInFavoriteGroup: boolean;
  autoFavorite: boolean;
  updateOnReuse: boolean;
  sort: ClipboardItemSort;
  itemActions: ItemAction[];
  itemActionOrder: ItemAction[];
}

export interface Display {
  textMaxLines: number;
  imageMaxHeight: number;
  fileMaxCount: number;
}

export interface Capture {
  text: boolean;
  html: boolean;
  rtf: boolean;
  image: boolean;
  files: boolean;
  maxTextMb: number;
  maxImageMb: number;
  order: CaptureKind[];
}

export interface Sensitive {
  collectSecrets: boolean;
  redactSecrets: boolean;
}

export interface Retention {
  value: number;
  unit: RetentionUnit;
}

export interface History {
  retention: Retention;
  maxCount: number;
  cleanupIntervalHours: number;
}

export interface Search {
  defaultFocus: boolean;
  clearOnHide: boolean;
}

export interface Window {
  position: WindowPosition;
  scrollToTopOnOpen: boolean;
  selectRangeOnOpen: WindowOpenRangeSelection;
  selectCategoryOnOpen: WindowOpenCategorySelection;
  selectGroupOnOpen: WindowOpenGroupSelection;
  lightweightMode: boolean;
  idleDestroySeconds: number;
}

export interface Preview {
  hoverEnabled: boolean;
  hoverDelayMs: PreviewHoverDelayMs;
  spaceEnabled: boolean;
}

export interface Feedback {
  copySound: boolean;
}

export interface Filters {
  excludedAppIds: string[];
}

export interface OnboardingLegacyImport {
  checked: boolean;
  imported: boolean;
  importTypes: OnboardingLegacyImportType[];
  importedAt: string | null;
}

export interface Onboarding {
  completed: boolean;
  lastStep: number;
  legacyImport: OnboardingLegacyImport;
}

export interface Clipboard {
  capture: Capture;
  content: Content;
  display: Display;
  sensitive: Sensitive;
  history: History;
  search: Search;
  window: Window;
  preview: Preview;
  feedback: Feedback;
  filters: Filters;
}

export interface Update {
  autoCheck: boolean;
  frequency: UpdateFrequency;
  includeBeta: boolean;
  includeNightly: boolean;
  lastCheckedAt: string | null;
  skippedVersion: string | null;
}

export interface WebDav {
  enabled: boolean;
  url: string;
  username: string;
  password: string;
  fileName: string;
}

export interface Settings {
  general: General;
  appearance: Appearance;
  shortcuts: Shortcuts;
  clipboard: Clipboard;
  onboarding: Onboarding;
  update: Update;
  webdav: WebDav;
}

export type SettingsPatch = DeepPartial<Settings>;

type DeepPartial<T> =
  T extends ReadonlyArray<infer U>
    ? ReadonlyArray<U>
    : T extends object
      ? { [K in keyof T]?: DeepPartial<T[K]> }
      : T;
