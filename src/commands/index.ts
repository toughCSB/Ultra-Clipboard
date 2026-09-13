import { invoke } from "@tauri-apps/api/core";
import { TAURI_COMMAND } from "@/constants/commands";
import i18n from "@/i18n";
import { settingsState } from "@/stores/settings";
import type {
  ClipboardAction,
  ClipboardApp,
  ClipboardGroupInput,
  ClipboardGroupRecord,
  ClipboardItemPage,
  ClipboardItemQuery,
  ClipboardKind,
  ClipboardSubKind,
  UpdateNoteResult,
} from "@/types/clipboard";
import type { Settings, SettingsPatch } from "@/types/settings";
import { getMessageApi, getModalApi } from "@/utils/feedback";
import { log } from "@/utils/log";
import { confirmClearClipboardItems } from "./confirmClearClipboardItems";

interface AppError {
  kind: string;
  message: string;
}

export interface PreviewAnchorRect {
  left: number;
  pointerY?: number;
  top: number;
  width: number;
  height: number;
}

export interface ContextSubmenuAnchor {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface ContextSubmenuGroupInput {
  checked: boolean;
  id: string;
  label: string;
}

export interface ContextMenuItemPayload {
  action: ClipboardAction;
  label: string;
  accelerator: string | null;
  groups?: ContextSubmenuGroupInput[];
}

export interface ContextMenuShowPayload {
  itemId: string;
  isFavorite: boolean;
  isPinned: boolean;
  groups: Array<Array<ContextMenuItemPayload>>;
}

export interface ShowContextSubmenuInput {
  action: ClipboardAction;
  anchor: ContextSubmenuAnchor;
  groups: ContextSubmenuGroupInput[];
  itemId: string;
}

export interface ClipboardPreviewState {
  requestId: number;
  sessionId: number;
  itemId: string;
  anchor: PreviewAnchorRect;
  scaleFactor: number;
  workArea: {
    x: number;
    y: number;
    width: number;
    height: number;
  };
  clipboardWindow: {
    x: number;
    y: number;
    width: number;
    height: number;
  } | null;
  layout: ClipboardPreviewLayout;
}

export interface ClipboardPreviewRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export type ClipboardPreviewPlacement = "right" | "left" | "bottom" | "top";

export interface ClipboardPreviewLayout {
  overlayRect: ClipboardPreviewRect;
  sourceRect: ClipboardPreviewRect;
  panelRect: ClipboardPreviewRect;
  placement: ClipboardPreviewPlacement;
}

export interface ClipboardPreviewFileEntry {
  path: string;
  name: string;
  isDir: boolean;
  isImage: boolean;
  exists: boolean;
  size: number | null;
  iconPath?: string;
}

export interface ClipboardPreviewPayload {
  id: string;
  kind: ClipboardKind;
  subKind: ClipboardSubKind | null;
  updatedAt: string;
  text: string | null;
  imagePath: string | null;
  imageWidth: number | null;
  imageHeight: number | null;
  size: number | null;
  isSensitive: boolean;
  imageExists: boolean;
  files: ClipboardPreviewFileEntry[];
  totalFiles: number;
}

export interface StorageUsage {
  totalBytes: number;
  databaseBytes: number;
  resourcesBytes: number;
  settingsBytes: number;
}

export interface CleanCacheResult {
  removedFiles: number;
  removedBytes: number;
  storageUsage: StorageUsage;
}

export interface StorageLocation {
  currentPath: string;
  defaultPath: string;
  isCustom: boolean;
}

export interface ChangeStorageLocationResult {
  location: StorageLocation;
  storageUsage: StorageUsage;
}

export type PreferenceDirectoryTarget = "data" | "logs";
export type BackupExportMode = "encrypted" | "plain";
export type BackupContainerMode = "encrypted" | "plain";
export type BackupReceiveSource = "dragDrop" | "openFile";
export type BackupImportStrategy = "merge" | "overwrite";

export interface ExportHistoryBackupOptions {
  mode: BackupExportMode;
  password?: string;
}

export interface ExportHistoryBackupResult {
  path: string;
  totalBytes: number;
  itemCount: number;
  textCount: number;
  imageCount: number;
  filesCount: number;
  resourceBytes: number;
  exportedAt: string;
  mode: BackupExportMode;
}

export interface InspectHistoryBackupInput {
  path: string;
  source?: BackupReceiveSource;
}

export interface ImportHistoryBackupInput {
  path: string;
  password?: string;
}

export interface ImportHistoryBackupOptions {
  strategy: BackupImportStrategy;
}

export interface ImportHistoryBackupResult {
  strategy: BackupImportStrategy;
  importedItems: number;
  skippedItems: number;
  importedResources: number;
  importedSettings: boolean;
  requiresRestart: boolean;
}

export interface BackupReceivedPayload {
  path: string;
  source: BackupReceiveSource;
  mode: BackupContainerMode;
}

export type WindowLifecyclePhase =
  | "notCreated"
  | "created"
  | "ready"
  | "visible"
  | "hiddenWarm"
  | "dormant"
  | "destroyPending"
  | "destroyed";

export interface WindowLifecycleSnapshot {
  label: string;
  phase: WindowLifecyclePhase;
  generation: number;
  visible: boolean;
  retainPolicy: "permanent" | "destroyWhenIdle";
  dirtyOwnerCount: number;
  keepaliveCount: number;
  hiddenForMs: number | null;
  lastActiveAgoMs: number;
}

export interface UpdateMetadata {
  currentVersion: string;
  version: string;
  date: string | null;
  body: string | null;
  target: string;
  downloadUrl: string;
  downloaded: boolean;
}

export interface AppUpdateStatus {
  currentVersion: string;
  update: UpdateMetadata | null;
}

export interface AdminLaunchStatus {
  configured: boolean;
  runningAsAdmin: boolean;
  taskReady: boolean;
}

export interface UpdateDownloadProgress {
  downloaded: number;
  total: number | null;
  progress: number | null;
}

export interface OnboardingLegacyDataDetection {
  found: boolean;
  favoriteItemCount: number;
  importableDatabase: string | null;
  importableItemCount: number;
  normalItemCount: number;
  databaseFiles: string[];
  checkedAt: string;
  path: string | null;
  scanMessages: string[];
}

export type LegacyImportSelection = "normal" | "favorite";

export interface OnboardingLegacyImportResult {
  importedAt: string;
  importedFavorite: number;
  importedNormal: number;
  imported: number;
  selectedTypes: LegacyImportSelection[];
  skipped: number;
}

const toAppError = (error: unknown): AppError => {
  if (
    typeof error === "object" &&
    error !== null &&
    "kind" in error &&
    "message" in error
  ) {
    return error as AppError;
  }

  return { kind: "Unknown", message: String(error) };
};

const call = async <T>(
  command: string,
  labelKey: string,
  args?: Record<string, unknown>,
): Promise<T> => {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    const appError = toAppError(error);

    log.error(`invoke ${command} failed`, appError);
    getMessageApi().error(
      i18n.t("commands:error", {
        label: i18n.t(labelKey),
        message: appError.message,
      }),
    );

    throw appError;
  }
};

export const getSettings = () => {
  return call<Settings>(
    TAURI_COMMAND.GET_SETTINGS,
    "commands:labels.loadSettings",
  );
};

export const getRunAsAdminStatus = () => {
  return call<AdminLaunchStatus>(
    TAURI_COMMAND.GET_RUN_AS_ADMIN_STATUS,
    "commands:labels.loadRunAsAdminStatus",
  );
};

export const setRunAsAdmin = (enabled: boolean) => {
  return call<Settings>(
    TAURI_COMMAND.SET_RUN_AS_ADMIN,
    "commands:labels.setRunAsAdmin",
    { enabled },
  );
};

export const restartAsAdmin = () => {
  return call<void>(
    TAURI_COMMAND.RESTART_AS_ADMIN,
    "commands:labels.restartAsAdmin",
  );
};

export const openOnboarding = () => {
  return call<void>(
    TAURI_COMMAND.OPEN_ONBOARDING,
    "commands:labels.openOnboarding",
  );
};

export const setOnboardingStep = (step: number) => {
  return call<Settings>(
    TAURI_COMMAND.SET_ONBOARDING_STEP,
    "commands:labels.saveOnboarding",
    { step },
  );
};

export const finishOnboarding = () => {
  return call<Settings>(
    TAURI_COMMAND.FINISH_ONBOARDING,
    "commands:labels.finishOnboarding",
  );
};

export const detectLegacyData = () => {
  return call<OnboardingLegacyDataDetection>(
    TAURI_COMMAND.DETECT_LEGACY_DATA,
    "commands:labels.detectLegacyData",
  );
};

export const importLegacyData = async (types: LegacyImportSelection[]) => {
  const result = await call<OnboardingLegacyImportResult>(
    TAURI_COMMAND.IMPORT_LEGACY_DATA,
    "commands:labels.importLegacyData",
    { types },
  );

  getMessageApi().success(
    i18n.t("commands:messages.legacyDataImported", {
      imported: result.imported,
      skipped: result.skipped,
    }),
  );

  return result;
};

export const suspendGlobalShortcuts = () => {
  return call<void>(
    TAURI_COMMAND.SUSPEND_GLOBAL_SHORTCUTS,
    "commands:labels.suspendGlobalShortcuts",
  );
};

export const resumeGlobalShortcuts = () => {
  return call<void>(
    TAURI_COMMAND.RESUME_GLOBAL_SHORTCUTS,
    "commands:labels.resumeGlobalShortcuts",
  );
};

export const updateSettings = (patch: SettingsPatch) => {
  return call<Settings>(
    TAURI_COMMAND.UPDATE_SETTINGS,
    "commands:labels.saveSettings",
    { patch },
  );
};

export const setSyncPeerSecret = (peerId: string, secret: string) => {
  return call<void>(
    TAURI_COMMAND.SET_SYNC_PEER_SECRET,
    "commands:labels.saveSyncPeer",
    { peerId, secret },
  );
};

export const deleteSyncPeerSecret = (peerId: string) => {
  return call<void>(
    TAURI_COMMAND.DELETE_SYNC_PEER_SECRET,
    "commands:labels.deleteSyncPeer",
    { peerId },
  );
};

export const testSyncPeerSecret = (peerId: string) => {
  return call<void>(
    TAURI_COMMAND.TEST_SYNC_PEER_SECRET,
    "commands:labels.testSyncPeer",
    { peerId },
  );
};

export const resetSettings = async () => {
  const settings = await call<Settings>(
    TAURI_COMMAND.RESET_SETTINGS,
    "commands:labels.resetSettings",
  );

  getMessageApi().success(i18n.t("commands:messages.settingsReset"));

  return settings;
};

export const openUpdateWindow = () => {
  return call<void>(
    TAURI_COMMAND.OPEN_UPDATE_WINDOW,
    "commands:labels.openUpdateWindow",
  );
};

export const getUpdateStatus = () => {
  return call<AppUpdateStatus>(
    TAURI_COMMAND.GET_UPDATE_STATUS,
    "commands:labels.loadUpdateStatus",
  );
};

export const checkForUpdates = () => {
  return call<AppUpdateStatus>(
    TAURI_COMMAND.CHECK_FOR_UPDATES,
    "commands:labels.checkForUpdates",
  );
};

export const downloadUpdate = (version: string) => {
  return call<UpdateMetadata>(
    TAURI_COMMAND.DOWNLOAD_UPDATE,
    "commands:labels.downloadUpdate",
    { version },
  );
};

export const installUpdate = (version: string) => {
  return call<void>(
    TAURI_COMMAND.INSTALL_UPDATE,
    "commands:labels.installUpdate",
    { version },
  );
};

export const skipUpdateVersion = (version: string) => {
  return call<AppUpdateStatus>(
    TAURI_COMMAND.SKIP_UPDATE_VERSION,
    "commands:labels.skipUpdateVersion",
    { version },
  );
};

export const getStorageUsage = () => {
  return call<StorageUsage>(
    TAURI_COMMAND.GET_STORAGE_USAGE,
    "commands:labels.loadStorageUsage",
  );
};

export const getStorageLocation = () => {
  return call<StorageLocation>(
    TAURI_COMMAND.GET_STORAGE_LOCATION,
    "commands:labels.loadStorageLocation",
  );
};

export const changeStorageLocation = async (targetParentDir: string) => {
  const result = await call<ChangeStorageLocationResult>(
    TAURI_COMMAND.CHANGE_STORAGE_LOCATION,
    "commands:labels.changeStorageLocation",
    { targetParentDir },
  );

  getMessageApi().success(i18n.t("commands:messages.storageLocationChanged"));

  return result;
};

export const resetStorageLocation = async () => {
  const result = await call<ChangeStorageLocationResult>(
    TAURI_COMMAND.RESET_STORAGE_LOCATION,
    "commands:labels.resetStorageLocation",
  );

  getMessageApi().success(i18n.t("commands:messages.storageLocationReset"));

  return result;
};

export const cleanResourceCache = async () => {
  const result = await call<CleanCacheResult>(
    TAURI_COMMAND.CLEAN_RESOURCE_CACHE,
    "commands:labels.cleanCache",
  );

  const messageKey =
    result.removedFiles === 0 && result.removedBytes === 0
      ? "commands:messages.cacheAlreadyClean"
      : "commands:messages.cacheCleaned";

  getMessageApi().success(
    i18n.t(messageKey, {
      count: result.removedFiles,
      size: formatCommandBytes(result.removedBytes),
    }),
  );

  return result;
};

export const openPreferenceDirectory = (target: PreferenceDirectoryTarget) => {
  return call<void>(
    TAURI_COMMAND.OPEN_PREFERENCE_DIRECTORY,
    "commands:labels.openDirectory",
    {
      target,
    },
  );
};

export const exportHistoryBackup = async (
  targetPath: string,
  options: ExportHistoryBackupOptions,
) => {
  const result = await call<ExportHistoryBackupResult>(
    TAURI_COMMAND.EXPORT_HISTORY_BACKUP,
    "commands:labels.exportBackup",
    {
      options,
      targetPath,
    },
  );

  getMessageApi().success(
    i18n.t("commands:messages.backupExported", {
      count: result.itemCount,
      size: formatCommandBytes(result.totalBytes),
    }),
  );

  return result;
};

export const inspectHistoryBackup = (input: InspectHistoryBackupInput) => {
  return call<BackupContainerMode>(
    TAURI_COMMAND.INSPECT_HISTORY_BACKUP,
    "commands:labels.inspectBackup",
    { input },
  );
};

export const takePendingBackup = async () => {
  try {
    return await invoke<BackupReceivedPayload | null>(
      TAURI_COMMAND.TAKE_PENDING_BACKUP,
    );
  } catch (error) {
    log.error("take pending backup failed", toAppError(error));

    return null;
  }
};

export const openPreferenceWithHighlight = (settingId: string) => {
  return call<void>(
    TAURI_COMMAND.OPEN_PREFERENCE_WITH_HIGHLIGHT,
    "commands:labels.openWindow",
    { settingId },
  );
};

export const takePendingPreferenceHighlight = async () => {
  try {
    return await invoke<string | null>(
      TAURI_COMMAND.TAKE_PENDING_PREFERENCE_HIGHLIGHT,
    );
  } catch (error) {
    log.error("take pending preference highlight failed", toAppError(error));

    return null;
  }
};

export const importHistoryBackup = async (
  input: ImportHistoryBackupInput,
  options: ImportHistoryBackupOptions,
) => {
  const result = await call<ImportHistoryBackupResult>(
    TAURI_COMMAND.IMPORT_HISTORY_BACKUP,
    "commands:labels.importBackup",
    {
      input,
      options,
    },
  );

  getMessageApi().success(
    i18n.t(
      result.strategy === "overwrite"
        ? "commands:messages.backupOverwriteImported"
        : "commands:messages.backupImported",
      {
        imported: result.importedItems,
        skipped: result.skippedItems,
      },
    ),
  );

  return result;
};

export interface WebDavSyncResult {
  bytes: number;
  remoteUrl: string;
}

export const pushWebdavBackup = async () => {
  const result = await call<WebDavSyncResult>(
    TAURI_COMMAND.PUSH_WEBDAV_BACKUP,
    "commands:labels.pushWebdavBackup",
  );

  getMessageApi().success(
    i18n.t("commands:messages.webdavPushed", {
      size: formatCommandBytes(result.bytes),
    }),
  );

  return result;
};

export const pullWebdavBackup = async () => {
  const result = await call<ImportHistoryBackupResult>(
    TAURI_COMMAND.PULL_WEBDAV_BACKUP,
    "commands:labels.pullWebdavBackup",
  );

  getMessageApi().success(
    i18n.t("commands:messages.webdavPulled", {
      imported: result.importedItems,
      skipped: result.skippedItems,
    }),
  );

  return result;
};

const formatCommandBytes = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;

  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value.toFixed(value >= 10 ? 1 : 2)} ${units[unitIndex]}`;
};

export const openExternalUrl = (url: string) => {
  return call<void>(
    TAURI_COMMAND.OPEN_EXTERNAL_URL,
    "commands:labels.openLink",
    { url },
  );
};

export const getAutostart = () => {
  return call<boolean>(
    TAURI_COMMAND.GET_AUTOSTART,
    "commands:labels.loadAutostart",
  );
};

export const setAutostart = (enabled: boolean) => {
  return call<void>(
    TAURI_COMMAND.SET_AUTOSTART,
    "commands:labels.setAutostart",
    {
      enabled,
    },
  );
};

export const listAllApps = () => {
  return call<ClipboardApp[]>(
    TAURI_COMMAND.LIST_ALL_APPS,
    "commands:labels.loadApps",
  );
};

export const addClipboardAppFromPath = (path: string) => {
  return call<ClipboardApp>(
    TAURI_COMMAND.ADD_CLIPBOARD_APP_FROM_PATH,
    "commands:labels.addApp",
    { path },
  );
};

export const deleteUnreferencedClipboardApps = (ids: string[]) => {
  return call<string[]>(
    TAURI_COMMAND.DELETE_UNREFERENCED_CLIPBOARD_APPS,
    "commands:labels.deleteApps",
    { ids },
  );
};

export const listClipboardItems = (query: ClipboardItemQuery) => {
  return call<ClipboardItemPage>(
    TAURI_COMMAND.LIST_CLIPBOARD_ITEMS,
    "commands:labels.loadClipboardList",
    { query },
  );
};

export const listClipboardGroups = () => {
  return call<ClipboardGroupRecord[]>(
    TAURI_COMMAND.LIST_CLIPBOARD_GROUPS,
    "commands:labels.loadClipboardGroups",
  );
};

export const createClipboardGroup = async (input: ClipboardGroupInput) => {
  const group = await call<ClipboardGroupRecord>(
    TAURI_COMMAND.CREATE_CLIPBOARD_GROUP,
    "commands:labels.saveClipboardGroup",
    { input },
  );

  getMessageApi().success(i18n.t("commands:messages.clipboardGroupSaved"));

  return group;
};

export const updateClipboardGroup = async (
  id: string,
  input: ClipboardGroupInput,
) => {
  await call<void>(
    TAURI_COMMAND.UPDATE_CLIPBOARD_GROUP,
    "commands:labels.saveClipboardGroup",
    { id, input },
  );

  getMessageApi().success(i18n.t("commands:messages.clipboardGroupSaved"));
};

export const updateClipboardGroupsLayout = async (
  order: string[],
  visibleIds: string[],
) => {
  await call<void>(
    TAURI_COMMAND.UPDATE_CLIPBOARD_GROUPS_LAYOUT,
    "commands:labels.saveClipboardGroupsLayout",
    { input: { order, visibleIds } },
  );

  getMessageApi().success(
    i18n.t("commands:messages.clipboardGroupsLayoutSaved"),
  );
};

export const deleteClipboardGroup = async (id: string) => {
  await call<void>(
    TAURI_COMMAND.DELETE_CLIPBOARD_GROUP,
    "commands:labels.deleteClipboardGroup",
    { id },
  );

  getMessageApi().success(i18n.t("commands:messages.clipboardGroupDeleted"));
};

export const updateClipboardItemGroup = async (id: string, groupId: string) => {
  await call<void>(
    TAURI_COMMAND.UPDATE_CLIPBOARD_ITEM_GROUP,
    "commands:labels.moveToGroup",
    { groupId, id },
  );

  getMessageApi().success(i18n.t("commands:messages.itemMovedToGroup"));
};

export const importClipboardGroupSvg = (path: string) => {
  return call<string>(
    TAURI_COMMAND.IMPORT_CLIPBOARD_GROUP_SVG,
    "commands:labels.importClipboardGroupSvg",
    { path },
  );
};

export const openClipboardItemLink = (id: string, mailto: boolean) => {
  return call<void>(
    TAURI_COMMAND.OPEN_CLIPBOARD_ITEM_LINK,
    "commands:labels.openLink",
    {
      id,
      mailto,
    },
  );
};

export const revealClipboardItem = (id: string) => {
  return call<void>(
    TAURI_COMMAND.REVEAL_CLIPBOARD_ITEM,
    "commands:labels.reveal",
    { id },
  );
};

export const saveClipboardImageToFile = async (id: string) => {
  const path = await call<string | null>(
    TAURI_COMMAND.SAVE_CLIPBOARD_IMAGE_TO_FILE,
    "commands:labels.saveImage",
    { id },
  );

  if (path !== null) {
    getMessageApi().success(i18n.t("commands:messages.imageSaved"));
  }

  return path;
};

export const writeToClipboard = async (id: string, plain: boolean) => {
  await call<void>(TAURI_COMMAND.WRITE_TO_CLIPBOARD, "commands:labels.copy", {
    id,
    plain,
  });

  getMessageApi().success(i18n.t("commands:messages.copied"));
};

export const pasteClipboardItem = (id: string, plain: boolean) => {
  return call<void>(
    TAURI_COMMAND.PASTE_CLIPBOARD_ITEM,
    "commands:labels.paste",
    { id, plain },
  );
};

export const startDragClipboardItem = (id: string) => {
  return call<void>(
    TAURI_COMMAND.START_DRAG_CLIPBOARD_ITEM,
    "commands:labels.drag",
    { id },
  );
};

export const toggleClipboardItemFavorite = async (
  id: string,
  favorite: boolean,
) => {
  const next = await call<boolean>(
    TAURI_COMMAND.TOGGLE_CLIPBOARD_ITEM_FAVORITE,
    favorite
      ? "commands:labels.toggleFavorite"
      : "commands:labels.cancelFavorite",
    { id },
  );

  getMessageApi().success(
    i18n.t(
      next
        ? "commands:messages.favoriteAdded"
        : "commands:messages.favoriteRemoved",
    ),
  );

  return next;
};

export const toggleClipboardItemPinned = async (
  id: string,
  pinned: boolean,
) => {
  const next = await call<boolean>(
    TAURI_COMMAND.TOGGLE_CLIPBOARD_ITEM_PINNED,
    pinned ? "commands:labels.pinItem" : "commands:labels.unpinItem",
    { id },
  );

  getMessageApi().success(
    i18n.t(
      next ? "commands:messages.itemPinned" : "commands:messages.itemUnpinned",
    ),
  );

  return next;
};

export const deleteClipboardItem = async (
  id: string,
  isFavorite: boolean,
  isPinned: boolean,
): Promise<boolean> => {
  const contentSettings = settingsState.clipboard?.content;

  if (isFavorite && !(contentSettings?.deleteFavoriteItems ?? false)) {
    return false;
  }

  if (isPinned && !(contentSettings?.deletePinnedItems ?? false)) {
    return false;
  }

  const needConfirm =
    (isFavorite && (contentSettings?.deleteFavoriteConfirm ?? true)) ||
    (isPinned && (contentSettings?.deletePinnedConfirm ?? true)) ||
    (!isFavorite && !isPinned && (contentSettings?.deleteConfirm ?? true));

  if (needConfirm) {
    const ok = await new Promise<boolean>((resolve) => {
      getModalApi().confirm({
        cancelText: i18n.t("common:actions.cancel"),
        centered: true,
        content: i18n.t("commands:deleteConfirm.content"),
        okButtonProps: { danger: true },
        okText: i18n.t("common:actions.delete"),
        onCancel: () => resolve(false),
        onOk: () => resolve(true),
        title: i18n.t("commands:deleteConfirm.title"),
      });
    });

    if (!ok) return false;
  }

  await call<void>(
    TAURI_COMMAND.DELETE_CLIPBOARD_ITEM,
    "commands:labels.delete",
    { id },
  );

  getMessageApi().success(i18n.t("commands:messages.deleted"));

  return true;
};

export const clearClipboardItems = async (): Promise<boolean> => {
  const options = await confirmClearClipboardItems();

  if (!options) return false;

  const removed = await call<number>(
    TAURI_COMMAND.CLEAR_CLIPBOARD_ITEMS,
    "commands:labels.clearClipboardItems",
    {
      deleteFavorites: options.deleteFavorites,
      deletePinned: options.deletePinned,
    },
  );

  getMessageApi().success(
    i18n.t("commands:messages.clipboardItemsCleared", { count: removed }),
  );

  return true;
};

export const updateClipboardItemNote = async (
  id: string,
  note: string | null,
) => {
  const result = await call<UpdateNoteResult>(
    TAURI_COMMAND.UPDATE_CLIPBOARD_ITEM_NOTE,
    "commands:labels.saveNote",
    { id, note },
  );

  getMessageApi().success(
    i18n.t(
      result.autoFavorited
        ? "commands:messages.noteSavedAndFavorited"
        : "commands:messages.noteSaved",
    ),
  );

  return result;
};

export const showWindow = (label: string) => {
  return call<void>(TAURI_COMMAND.SHOW_WINDOW, "commands:labels.openWindow", {
    label,
  });
};

export const hideWindow = (label: string) => {
  return call<void>(TAURI_COMMAND.HIDE_WINDOW, "commands:labels.closeWindow", {
    label,
  });
};

export const notifyWindowReady = async (label: string) => {
  try {
    await invoke<void>(TAURI_COMMAND.NOTIFY_WINDOW_READY, { label });
  } catch (error) {
    log.error("notify window ready failed", toAppError(error));
  }
};

export const setWindowDirty = async (
  label: string,
  owner: string,
  dirty: boolean,
) => {
  try {
    await invoke<void>(TAURI_COMMAND.SET_WINDOW_DIRTY, {
      dirty,
      label,
      owner,
    });
  } catch (error) {
    log.error("set window dirty failed", toAppError(error));
  }
};

export const acquireWindowKeepalive = async (
  label: string,
  owner: string,
  reason: string,
  timeoutMs?: number,
) => {
  try {
    await invoke<void>(TAURI_COMMAND.ACQUIRE_WINDOW_KEEPALIVE, {
      label,
      owner,
      reason,
      timeoutMs,
    });
  } catch (error) {
    log.error("acquire window keepalive failed", toAppError(error));
  }
};

export const releaseWindowKeepalive = async (label: string, owner: string) => {
  try {
    await invoke<void>(TAURI_COMMAND.RELEASE_WINDOW_KEEPALIVE, {
      label,
      owner,
    });
  } catch (error) {
    log.error("release window keepalive failed", toAppError(error));
  }
};

export const getWindowLifecycleSnapshot = async () => {
  try {
    return await invoke<WindowLifecycleSnapshot[]>(
      TAURI_COMMAND.GET_WINDOW_LIFECYCLE_SNAPSHOT,
    );
  } catch (error) {
    log.error("get window lifecycle snapshot failed", toAppError(error));

    return [];
  }
};

export const showTaskbarIcon = (visible: boolean) => {
  return call<void>(
    TAURI_COMMAND.SHOW_TASKBAR_ICON,
    "commands:labels.setTaskbarIcon",
    {
      visible,
    },
  );
};

export const setClipboardWindowPinned = (pinned: boolean) => {
  return call<void>(
    TAURI_COMMAND.SET_CLIPBOARD_WINDOW_PINNED,
    "commands:labels.setClipboardWindowPinned",
    {
      pinned,
    },
  );
};

export const setClipboardWindowAutoHideSuspended = (suspended: boolean) => {
  return call<void>(
    TAURI_COMMAND.SET_CLIPBOARD_WINDOW_AUTO_HIDE_SUSPENDED,
    "commands:labels.setClipboardWindowAutoHideSuspended",
    {
      suspended,
    },
  );
};

export const setClipboardWindowEditing = async (editing: boolean) => {
  try {
    await invoke<void>(TAURI_COMMAND.SET_CLIPBOARD_WINDOW_EDITING, {
      editing,
    });
  } catch (error) {
    log.error("set clipboard window editing failed", toAppError(error));
  }
};

export const showClipboardPreview = (
  itemId: string,
  anchor: PreviewAnchorRect,
) => {
  return call<ClipboardPreviewState | null>(
    TAURI_COMMAND.SHOW_CLIPBOARD_PREVIEW,
    "commands:labels.openPreview",
    { anchor, itemId },
  );
};

export const closeClipboardPreview = () => {
  return call<void>(
    TAURI_COMMAND.CLOSE_CLIPBOARD_PREVIEW,
    "commands:labels.closePreview",
  );
};

export const getClipboardPreviewState = () => {
  return call<ClipboardPreviewState | null>(
    TAURI_COMMAND.GET_CLIPBOARD_PREVIEW_STATE,
    "commands:labels.loadPreviewState",
  );
};

export const getClipboardPreviewPayload = (itemId: string) => {
  return call<ClipboardPreviewPayload | null>(
    TAURI_COMMAND.GET_CLIPBOARD_PREVIEW_PAYLOAD,
    "commands:labels.loadPreviewContent",
    { itemId },
  );
};

export const playCopySound = () => {
  return call<void>(
    TAURI_COMMAND.PLAY_COPY_SOUND,
    "commands:labels.playCopySound",
  );
};

export const popupClipboardItemMenu = (
  itemId: string,
  availableActions: ClipboardAction[],
  currentGroupId: string | null,
  isFavorite: boolean,
  isPinned: boolean,
  hasNote: boolean,
) => {
  return call<void>(
    TAURI_COMMAND.POPUP_CLIPBOARD_ITEM_MENU,
    "commands:labels.openMenu",
    {
      input: {
        availableActions,
        currentGroupId,
        hasNote,
        isFavorite,
        isPinned,
        itemId,
      },
    },
  );
};

export const getContextMenuPayload = async () => {
  try {
    return await invoke<ContextMenuShowPayload | null>(
      TAURI_COMMAND.GET_CONTEXT_MENU_PAYLOAD,
    );
  } catch (error) {
    log.error("get context menu payload failed", toAppError(error));

    return null;
  }
};

export const getContextSubmenuPayload = async () => {
  try {
    return await invoke<ShowContextSubmenuInput | null>(
      TAURI_COMMAND.GET_CONTEXT_SUBMENU_PAYLOAD,
    );
  } catch (error) {
    log.error("get context submenu payload failed", toAppError(error));

    return null;
  }
};

export const showContextSubmenu = async (input: ShowContextSubmenuInput) => {
  try {
    await invoke<void>(TAURI_COMMAND.SHOW_CONTEXT_SUBMENU, { input });
  } catch (error) {
    log.error("show context submenu failed", toAppError(error));
  }
};

export const hideContextSubmenu = async () => {
  try {
    await invoke<void>(TAURI_COMMAND.HIDE_CONTEXT_SUBMENU);
  } catch (error) {
    log.error("hide context submenu failed", toAppError(error));
  }
};

export const hideContextMenus = async () => {
  try {
    await invoke<void>(TAURI_COMMAND.HIDE_CONTEXT_MENUS);
  } catch (error) {
    log.error("hide context menus failed", toAppError(error));
  }
};
