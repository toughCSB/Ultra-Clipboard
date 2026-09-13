/** Frontend shape corresponding to Rust `db::models::ClipboardItem`. */
export type ClipboardKind = "text" | "image" | "files";

export type ClipboardSubKind =
  | "rtf"
  | "html"
  | "url"
  | "email"
  | "color"
  | "path";

export type ClipboardPlatform = "macos" | "windows";
/** Context-menu actions returned by Rust, in the order they should be rendered. */
export type ClipboardAction =
  | "paste"
  | "pasteAsPlainText"
  | "pasteAsPath"
  | "copy"
  | "saveImage"
  | "openLink"
  | "sendEmail"
  | "revealInFinder"
  | "revealInExplorer"
  | "toggleFavorite"
  | "togglePinned"
  | "moveToGroup"
  | "editNote"
  | "delete";

export interface ClipboardItem {
  id: string;
  kind: ClipboardKind;
  subKind: ClipboardSubKind | null;
  groupId: string | null;
  sourceAppId: string | null;
  content: string;
  contentHash: string;
  searchText: string | null;
  summary: string | null;
  fileTypes: string | null;
  size: number | null;
  width: number | null;
  height: number | null;
  useCount: number;
  isFavorite: boolean;
  isPinned: boolean;
  isSensitive: boolean;
  platform: ClipboardPlatform;
  note: string | null;
  createdAt: string;
  updatedAt: string;
  /** Source-app metadata returned by the backend's `clipboard_apps` join. */
  sourceAppName?: string;
  sourceAppIconFile?: string;
  sourceAppIconPath?: string;
  /** Absolute thumbnail path prepared by the backend for image items. */
  imageThumbnailPath?: string;
  /** Preprocessed file entries, in the same order as the content paths. */
  fileEntries?: FileEntry[];
  /** Context-menu actions returned by Rust in render order. */
  availableActions?: ClipboardAction[];
  /** Validated CSS color string for color subtypes. */
  colorPreview?: string;
  /** Files-card rendering mode calculated by the Rust command layer. */
  filesPreviewKind?: "imagePreview" | "list";
  /** `createdAt` formatted in the local timezone for display. */
  displayCreatedAt?: string;
}

export interface ClipboardApp {
  id: string;
  name: string;
  iconFile: string | null;
  iconPath: string | null;
  platform: ClipboardPlatform;
  createdAt: string;
  updatedAt: string;
}

export interface FileEntry {
  path: string;
  name: string;
  isDir: boolean;
  isImage: boolean;
  exists: boolean;
  iconPath?: string;
}

export type ClipboardItemSort =
  | "createdAtDesc"
  | "updatedAtDesc"
  | "useCountDesc";

export type ClipboardGroup = "all" | "text" | "image" | "files" | "favorite";

export type ClipboardRange = "all" | "favorite";

export type ClipboardCategory = ClipboardKind;

export type ClipboardGroupIcon = string;

export interface ClipboardGroupRecord {
  id: string;
  name: string;
  icon: ClipboardGroupIcon;
  isHidden: boolean;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface ClipboardGroupInput {
  name: string;
  icon: ClipboardGroupIcon;
  isHidden: boolean;
}

export interface ClipboardItemQuery {
  kind?: ClipboardKind;
  groupId?: string;
  favorite?: boolean;
  pinned?: boolean;
  /** Top-level list tab; Rust maps it to kind or favorite filtering. */
  group?: ClipboardGroup;
  keyword?: string;
  sort?: ClipboardItemSort;
  limit?: number;
  offset?: number;
}
/** One page returned by the Rust list query, including its total and continuation flag. */
export interface ClipboardItemPage {
  list: ClipboardItem[];
  total: number;
  hasMore: boolean;
}
/** Normalized note update result and whether auto-favorite was triggered. */
export interface UpdateNoteResult {
  note: string | null;
  autoFavorited: boolean;
}
