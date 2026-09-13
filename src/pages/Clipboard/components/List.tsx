import { useMount } from "ahooks";
import { Empty, Spin } from "antd";
import type { TFunction } from "i18next";
import type {
  FC,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
} from "react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  type TopItemListProps,
  Virtuoso,
  type VirtuosoHandle,
} from "react-virtuoso";
import { useSnapshot } from "valtio";
import {
  deleteClipboardItem,
  hideWindow,
  listClipboardGroups,
  openClipboardItemLink,
  pasteClipboardItem,
  revealClipboardItem,
  saveClipboardImageToFile,
  toggleClipboardItemFavorite,
  toggleClipboardItemPinned,
  updateClipboardItemGroup,
  writeToClipboard,
} from "@/commands";
import VirtuosoScroller, {
  type VirtuosoScrollerChildrenProps,
} from "@/components/VirtuosoScroller";
import { TAURI_EVENT } from "@/constants/events";
import { buildItemActionLabels } from "@/constants/itemActions";
import {
  parseWindowOpenGroupId,
  WINDOW_OPEN_SELECTION_ALL,
  WINDOW_OPEN_SELECTION_PRESERVE,
} from "@/constants/windowOpenSelection";
import { WINDOW_LABEL } from "@/constants/windows";
import { useClipboardItems } from "@/hooks/useClipboardItems";
import { useKeyboardEvent } from "@/hooks/useKeyboardEvent";
import { useTauriListen } from "@/hooks/useTauriListen";
import { clipboardStatsState } from "@/stores/clipboardStats";
import { clipboardViewState } from "@/stores/clipboardView";
import { settingsState } from "@/stores/settings";
import type {
  ClipboardAction,
  ClipboardGroupRecord,
  ClipboardItem,
  ClipboardKind,
  ClipboardRange,
} from "@/types/clipboard";
import type { ItemAction } from "@/types/settings";
import { cn } from "@/utils/cn";
import { isMac } from "@/utils/is";
import type { WindowVisibilityPayload } from "../hooks/previewController";
import {
  isSpaceKey,
  useClipboardPreviewController,
} from "../hooks/useClipboardPreviewController";
import ClipboardCard from "./cards/ClipboardCard";
import NoteModal from "./NoteModal";

/** Shortcuts for the first ten items, 1 through 9 followed by 0. */
const KEY_HINTS = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];

interface ClipboardUpdatedPayload {
  cleanup?: number;
  deduplicated?: boolean;
  id?: string;
  imported?: boolean;
  kind?: ClipboardKind;
}

interface ClipboardMenuActionPayload {
  action: ClipboardAction;
  groupId?: string;
  itemId: string;
}

/** Clipboard history list with virtual scrolling and range-based pagination. */
const List: FC = () => {
  const { t } = useTranslation("clipboard");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [firstVisibleIndex, setFirstVisibleIndex] = useState(0);
  const [isModifierPressed, setIsModifierPressed] = useState(false);
  const [customGroups, setCustomGroups] = useState<ClipboardGroupRecord[]>([]);
  const [noteTarget, setNoteTarget] = useState<ClipboardItem | null>(null);
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const isAtTopRef = useRef(true);
  const itemElementMapRef = useRef(new Map<string, HTMLDivElement>());
  const closePreviewRef = useRef<(reason: string) => void>(() => {});
  const displaySettingsMountedRef = useRef(false);
  const keywordRef = useRef("");
  const reloadCurrentRangeRef = useRef<() => void>(() => {});
  const deferredReloadRef = useRef(false);

  const clipboardWindowVisibleRef = useRef(false);

  const snapshot = useSnapshot(clipboardViewState);
  const settings = useSnapshot(settingsState);
  const { category, keyword, groupId, range } = snapshot;
  const autoPaste = settings.clipboard.content.autoPaste;
  const middleClick = settings.clipboard.content.middleClick;
  const display = settings.clipboard.display;
  const sort = settings.clipboard.content.sort;
  const redactSecrets = settings.clipboard.sensitive.redactSecrets;
  const quickActions = settings.clipboard.content.itemActions;
  const deleteFavoriteItems = settings.clipboard.content.deleteFavoriteItems;
  const deletePinnedItems = settings.clipboard.content.deletePinnedItems;
  const deleteFavoriteItemsOnlyInFavoriteGroup =
    settings.clipboard.content.deleteFavoriteItemsOnlyInFavoriteGroup;
  const { fileMaxCount } = display;
  const showOriginalPreview = settings.clipboard.content.showOriginalPreview;
  const quickActionLabels = buildItemActionLabels(t);
  const currentGroupName = getCurrentGroupName(customGroups, groupId);

  const {
    findItemById,
    getItem,
    getItemIndexById,
    loadRange,
    loadedInitial,
    loading,
    patchItemById,
    reload,
    reloadCurrentRange,
    removeItemById,
    total,
  } = useClipboardItems({
    favorite: range === "favorite" ? true : void 0,
    groupId: groupId ?? void 0,
    keyword,
    kind: category ?? void 0,
    sort,
  });
  const topItemCount = countLeadingPinnedItems(getItem);
  const {
    closeHoverPreviewForScroll,
    closePreview,
    handleItemPointerEnter,
    handleItemPointerLeave,
    handleItemPointerMove,
    handleKeyboardPreviewMove,
    handlePreviewAreaPointerLeave,
    handlePreviewSpaceDown,
    previewSession,
  } = useClipboardPreviewController({
    getActiveItem,
    itemElementMapRef,
    onHoverSelect: setSelectedId,
  });
  closePreviewRef.current = closePreview;
  reloadCurrentRangeRef.current = reloadCurrentRange;

  // Keep clipboard updates pending while the hidden window is dormant.
  useEffect(() => {
    if (loadedInitial) clipboardStatsState.total = total;
  }, [loadedInitial, total]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: snapshot is the trigger for filter changes.
  useEffect(() => {
    setSelectedId(null);
    if (keywordRef.current !== keyword) keywordRef.current = keyword;
    deferredReloadRef.current = false;
    isAtTopRef.current = true;
    closePreview("filterChange");
  }, [snapshot]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: only display settings affecting item payloads trigger reload; refs provide current callbacks.
  useEffect(() => {
    if (!displaySettingsMountedRef.current) {
      displaySettingsMountedRef.current = true;
      return;
    }

    closePreviewRef.current("displaySettingChange");
    reloadCurrentRangeRef.current();
  }, [fileMaxCount, redactSecrets]);

  const loadGroups = async () => {
    const groups = await listClipboardGroups();

    setCustomGroups(groups);
  };

  useMount(() => {
    void loadGroups();
  });

  const handleGroupsUpdated = () => {
    void loadGroups();
  };

  useTauriListen(TAURI_EVENT.CLIPBOARD_GROUPS_UPDATED, handleGroupsUpdated);

  const handleClipboardUpdated = (payload: ClipboardUpdatedPayload) => {
    if (!clipboardWindowVisibleRef.current) {
      deferredReloadRef.current = true;
      return;
    }

    if (payload.cleanup !== void 0) {
      closePreview("cleanup");
      setSelectedId(null);
      deferredReloadRef.current = false;
      if (clipboardStatsState.total !== null) {
        clipboardStatsState.total = Math.max(
          clipboardStatsState.total - payload.cleanup,
          0,
        );
      }
      requestReloadAtTop();
      return;
    }

    if (payload.imported) {
      closePreview("backupImport");
      setSelectedId(null);
      requestReloadAtTop();
      return;
    }

    if (payload.deduplicated) {
      if (
        !shouldRefreshCurrentGroup(
          clipboardViewState.range,
          clipboardViewState.category,
          clipboardViewState.groupId,
          payload.kind,
        )
      ) {
        return;
      }

      requestReloadAtTop();
      return;
    }

    if (
      !shouldRefreshCurrentGroup(
        clipboardViewState.range,
        clipboardViewState.category,
        clipboardViewState.groupId,
        payload.kind,
      )
    ) {
      return;
    }

    requestReloadAtTop();
  };

  useTauriListen<ClipboardUpdatedPayload>(
    TAURI_EVENT.CLIPBOARD_UPDATED,
    (event) => {
      handleClipboardUpdated(event.payload);
    },
  );

  const handleWindowVisibility = (event: {
    payload: WindowVisibilityPayload;
  }) => {
    const { label, visible } = event.payload;
    if (label !== WINDOW_LABEL.CLIPBOARD) return;

    clipboardWindowVisibleRef.current = visible;
    if (!visible) return;

    const {
      scrollToTopOnOpen,
      selectCategoryOnOpen,
      selectGroupOnOpen,
      selectRangeOnOpen,
    } = settings.clipboard.window;
    const shouldResetSelection =
      selectRangeOnOpen !== WINDOW_OPEN_SELECTION_PRESERVE ||
      selectCategoryOnOpen !== WINDOW_OPEN_SELECTION_PRESERVE ||
      selectGroupOnOpen !== WINDOW_OPEN_SELECTION_PRESERVE;
    if (!scrollToTopOnOpen && !shouldResetSelection) return;

    closePreview("windowOpenReset");

    if (selectRangeOnOpen !== WINDOW_OPEN_SELECTION_PRESERVE) {
      clipboardViewState.range = selectRangeOnOpen;
    }

    if (selectCategoryOnOpen === WINDOW_OPEN_SELECTION_ALL) {
      clipboardViewState.category = null;
    } else if (selectCategoryOnOpen !== WINDOW_OPEN_SELECTION_PRESERVE) {
      clipboardViewState.category = selectCategoryOnOpen;
    }

    const openGroupId = parseWindowOpenGroupId(selectGroupOnOpen);
    if (selectGroupOnOpen === WINDOW_OPEN_SELECTION_ALL) {
      clipboardViewState.groupId = null;
    } else if (openGroupId) {
      clipboardViewState.groupId = openGroupId;
    }

    if (!scrollToTopOnOpen) return;

    setSelectedId(null);
    virtuosoRef.current?.scrollToIndex({ behavior: "auto", index: 0 });
    consumeDeferredReloadAtTop();
  };

  useTauriListen<WindowVisibilityPayload>(
    TAURI_EVENT.WINDOW_VISIBILITY,
    handleWindowVisibility,
  );

  const removeItem = (id: string) => {
    removeItemById(id);
  };

  const patchItem = (id: string, patch: Partial<ClipboardItem>) => {
    patchItemById(id, patch);
  };

  const handleFavoriteToggled = (id: string, isFavorite: boolean) => {
    if (range === "favorite" && !isFavorite) {
      removeItem(id);
      return;
    }

    patchItem(id, { isFavorite });
  };

  const handleNoteSaved = (
    id: string,
    note: string | null,
    autoFavorited: boolean,
  ) => {
    patchItem(id, autoFavorited ? { isFavorite: true, note } : { note });
  };

  const handleCloseNote = () => {
    setNoteTarget(null);
  };

  const handleOpenNote = (item: ClipboardItem, reason: string) => {
    if (previewSession?.itemId === item.id) closePreview(reason);

    setNoteTarget(item);
  };

  const handleMoveToGroup = async (
    item: ClipboardItem,
    nextGroupId: string,
  ) => {
    if (previewSession?.itemId === item.id) closePreview("moveToGroup");

    await updateClipboardItemGroup(item.id, nextGroupId);

    if (groupId !== null && groupId !== nextGroupId) {
      removeItem(item.id);
      return;
    }

    patchItem(item.id, { groupId: nextGroupId });
  };

  function getActiveItem() {
    if (total === 0) return null;

    if (selectedId === null) {
      return getItem(firstVisibleIndex) ?? getItem(0);
    }

    return findItemById(selectedId);
  }

  const registerItemElement = (id: string) => {
    return (node: HTMLDivElement | null) => {
      if (node) {
        itemElementMapRef.current.set(id, node);
        return;
      }

      itemElementMapRef.current.delete(id);
    };
  };

  const handleShortcutDelete = async (id: string) => {
    const target = findItemById(id);

    if (!target || !canDeleteItem(target)) return;

    if (previewSession?.itemId === id) closePreview("delete");

    const deleted = await deleteClipboardItem(
      id,
      target.isFavorite,
      target.isPinned,
    );

    if (!deleted) return;

    setSelectedId(
      getSelectedIdAfterDelete(
        getItem,
        getItemIndexById,
        firstVisibleIndex,
        selectedId,
        id,
      ),
    );
    removeItem(id);
  };

  const handleShortcutToggleFavorite = async (id: string) => {
    const current = findItemById(id);

    if (!current) return;

    const next = await toggleClipboardItemFavorite(id, !current.isFavorite);

    handleFavoriteToggled(id, next);
  };

  const handleTogglePinned = async (id: string) => {
    const current = findItemById(id);

    if (!current) return;

    const next = await toggleClipboardItemPinned(id, !current.isPinned);

    patchItem(id, { isPinned: next });
    reloadCurrentRange();
  };

  const handleShortcutOpen = async (
    item: ClipboardItem,
    action: ClipboardAction,
  ) => {
    if (previewSession?.itemId === item.id) closePreview("shortcutOpen");

    switch (action) {
      case "openLink":
        await openClipboardItemLink(item.id, false);
        return;
      case "sendEmail":
        await openClipboardItemLink(item.id, true);
        return;
      case "revealInFinder":
      case "revealInExplorer":
        await revealClipboardItem(item.id);
        return;
      default:
        return;
    }
  };

  const handleMenuActionRef = useRef<
    (payload: ClipboardMenuActionPayload) => void
  >(() => {});
  handleMenuActionRef.current = (payload) => {
    const { action, groupId: targetGroupId, itemId } = payload;
    const target = findItemById(itemId);

    if (!target) return;

    switch (action) {
      case "paste":
        closePreview("paste");
        pasteClipboardItem(target.id, false);
        return;
      case "pasteAsPlainText":
      case "pasteAsPath":
        closePreview("pastePlain");
        pasteClipboardItem(target.id, true);
        return;
      case "copy":
        if (previewSession?.itemId === target.id) closePreview("copy");
        writeToClipboard(target.id, false);
        return;
      case "saveImage":
        if (previewSession?.itemId === target.id) closePreview("saveImage");
        saveClipboardImageToFile(target.id);
        return;
      case "openLink":
        if (previewSession?.itemId === target.id) closePreview("openLink");
        openClipboardItemLink(target.id, false);
        return;
      case "sendEmail":
        if (previewSession?.itemId === target.id) closePreview("sendEmail");
        openClipboardItemLink(target.id, true);
        return;
      case "revealInFinder":
      case "revealInExplorer":
        if (previewSession?.itemId === target.id) closePreview("reveal");
        revealClipboardItem(target.id);
        return;
      case "toggleFavorite":
        handleShortcutToggleFavorite(target.id);
        return;
      case "togglePinned":
        handleTogglePinned(target.id);
        return;
      case "moveToGroup":
        if (!targetGroupId) return;

        void handleMoveToGroup(target, targetGroupId);
        return;
      case "editNote":
        handleOpenNote(target, "editNote");
        return;
      case "delete":
        if (!canDeleteItem(target)) return;

        handleShortcutDelete(target.id);
        return;
    }
  };

  const handleMenuActionEvent = (event: { payload: unknown }) => {
    handleMenuActionRef.current(event.payload as ClipboardMenuActionPayload);
  };

  useTauriListen(TAURI_EVENT.CLIPBOARD_MENU_ACTION, handleMenuActionEvent);

  const handleKeyDown = (event: KeyboardEvent) => {
    const eventModifierPressed = isMac ? event.metaKey : event.ctrlKey;

    setIsModifierPressed(eventModifierPressed);

    if (event.key === "Escape") {
      event.preventDefault();
      closeTopEscapeLayer();

      return;
    }

    if (total === 0) return;

    if (event.key === "Enter") {
      event.preventDefault();

      const activeItem = getActiveItem();

      if (!activeItem) return;

      closePreview("enterPaste");
      pasteClipboardItem(activeItem.id, eventModifierPressed);

      return;
    }

    if (isSpaceKey(event)) {
      handlePreviewSpaceDown(event);
      return;
    }

    if (
      eventModifierPressed &&
      (event.key === "Backspace" || event.key === "Delete")
    ) {
      event.preventDefault();

      const activeItem = getActiveItem();

      if (!activeItem) return;

      handleShortcutDelete(activeItem.id);

      return;
    }

    if (
      eventModifierPressed &&
      event.key.toLowerCase() === "c" &&
      !shouldUseNativeCopy(event)
    ) {
      event.preventDefault();

      const activeItem = getActiveItem();

      if (!activeItem) return;

      if (previewSession?.itemId === activeItem.id)
        closePreview("shortcutCopy");

      writeToClipboard(activeItem.id, false);

      return;
    }

    if (eventModifierPressed && event.key.toLowerCase() === "o") {
      const activeItem = getActiveItem();

      if (!activeItem) return;

      const openAction = getOpenClipboardAction(activeItem.availableActions);

      if (!openAction) return;

      event.preventDefault();
      void handleShortcutOpen(activeItem, openAction);

      return;
    }

    if (eventModifierPressed && event.key.toLowerCase() === "d") {
      event.preventDefault();

      const activeItem = getActiveItem();

      if (!activeItem) return;

      handleShortcutToggleFavorite(activeItem.id);

      return;
    }

    if (eventModifierPressed && event.key.toLowerCase() === "t") {
      event.preventDefault();

      const activeItem = getActiveItem();

      if (!activeItem) return;

      handleTogglePinned(activeItem.id);

      return;
    }

    if (eventModifierPressed && event.key.toLowerCase() === "m") {
      event.preventDefault();

      const activeItem = getActiveItem();

      if (!activeItem) return;

      handleOpenNote(activeItem, "shortcutNote");

      return;
    }

    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;

    event.preventDefault();

    const next = getNextKeyboardTarget(event);

    if (!next) return;

    setSelectedId(next.item.id);
    virtuosoRef.current?.scrollIntoView({
      behavior: "smooth",
      index: next.index,
    });

    handleKeyboardPreviewMove(next.item);
  };

  useKeyboardEvent("keydown", handleKeyDown);

  const handleKeyUp = (event: KeyboardEvent) => {
    const eventModifierPressed = isMac ? event.metaKey : event.ctrlKey;

    setIsModifierPressed(eventModifierPressed);
  };

  useKeyboardEvent("keyup", handleKeyUp);

  const handleRangeChanged = ({
    endIndex,
    startIndex,
  }: {
    startIndex: number;
    endIndex: number;
  }) => {
    setFirstVisibleIndex(startIndex);

    closeHoverPreviewForScroll();
    loadRange(startIndex, endIndex);
  };

  const handleAtTopStateChange = (atTop: boolean) => {
    isAtTopRef.current = atTop;

    if (!atTop) return;

    consumeDeferredReloadAtTop();
  };

  function requestReloadAtTop() {
    if (!isAtTopRef.current && total > 0) {
      deferredReloadRef.current = true;
      return;
    }

    deferredReloadRef.current = false;
    reload();
  }

  function consumeDeferredReloadAtTop() {
    if (!deferredReloadRef.current) return;

    requestReloadAtTop();
  }

  if (loading && !loadedInitial) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center">
        <Spin />
      </div>
    );
  }

  const showEmpty = loadedInitial && total === 0 && !loading;
  const emptyDescription = getEmptyDescription(
    t,
    keyword,
    range,
    category,
    groupId,
    currentGroupName,
  );

  return (
    <div
      className="relative min-h-0 flex-1 overflow-hidden bg-slate-100 dark:bg-neutral-900"
      onPointerLeave={handlePreviewAreaPointerLeave}
      role="listbox"
    >
      {showEmpty ? (
        <div
          className="flex h-full flex-col items-center justify-center"
          data-tauri-drag-region
        >
          <Empty
            description={emptyDescription}
            image={Empty.PRESENTED_IMAGE_SIMPLE}
          />
        </div>
      ) : (
        <VirtuosoScroller>{renderVirtuoso}</VirtuosoScroller>
      )}

      <NoteModal
        item={noteTarget}
        onClose={handleCloseNote}
        onSaved={handleNoteSaved}
      />
    </div>
  );

  function renderVirtuoso(props: VirtuosoScrollerChildrenProps) {
    const { scrollerRef } = props;

    return (
      <Virtuoso
        atTopStateChange={handleAtTopStateChange}
        components={{ TopItemList }}
        computeItemKey={computeItemKey}
        itemContent={renderItemContent}
        rangeChanged={handleRangeChanged}
        ref={virtuosoRef}
        scrollerRef={scrollerRef}
        topItemCount={topItemCount}
        totalCount={total}
      />
    );
  }

  function renderItemContent(index: number) {
    const item = getItem(index);
    if (!item) return renderPlaceholderItem(index);

    const handlePointerEnter = (event: ReactPointerEvent<HTMLDivElement>) => {
      handleItemPointerEnter(item, event);
    };

    const handlePointerLeave = () => {
      handleItemPointerLeave();
    };

    const handlePointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
      handleItemPointerMove(item, event);
    };

    const relativeIndex = index - firstVisibleIndex;
    const hintKey =
      relativeIndex >= 0 && relativeIndex < 10
        ? KEY_HINTS[relativeIndex]
        : void 0;

    const handleQuickPaste = () => {
      closePreview("quickPaste");
      pasteClipboardItem(item.id, false);
    };

    const handleOpenLink = () => {
      closePreview("openLink");
      openClipboardItemLink(item.id, item.subKind === "email");
    };

    const handleEditNote = () => {
      handleOpenNote(item, "editNote");
    };

    const handleQuickAction = async (action: ItemAction) => {
      if (action === "delete" && !canDeleteItem(item)) return;

      switch (action) {
        case "paste":
          closePreview("quickPaste");
          await pasteClipboardItem(item.id, false);
          return;
        case "pastePlain":
          closePreview("quickPastePlain");
          await pasteClipboardItem(item.id, true);
          return;
        case "pastePath":
          closePreview("quickPastePath");
          await pasteClipboardItem(item.id, true);
          return;
        case "copy":
          if (previewSession?.itemId === item.id) closePreview("quickCopy");
          await writeToClipboard(item.id, false);
          return;
        case "copyPlain":
          if (previewSession?.itemId === item.id) {
            closePreview("quickCopyPlain");
          }
          await writeToClipboard(item.id, true);
          return;
        case "openLink":
          if (previewSession?.itemId === item.id) {
            closePreview("quickOpenLink");
          }
          await openClipboardItemLink(item.id, false);
          return;
        case "sendEmail":
          if (previewSession?.itemId === item.id) {
            closePreview("quickSendEmail");
          }
          await openClipboardItemLink(item.id, true);
          return;
        case "reveal":
          if (previewSession?.itemId === item.id) closePreview("quickReveal");
          await revealClipboardItem(item.id);
          return;
        case "note":
          handleEditNote();
          return;
        case "pinItem":
          await handleTogglePinned(item.id);
          return;
        case "star":
          await handleShortcutToggleFavorite(item.id);
          return;
        case "delete":
          await handleShortcutDelete(item.id);
          return;
      }
    };

    const handleMouseDown = (event: ReactMouseEvent<HTMLDivElement>) => {
      if (event.button !== 0) {
        if (event.button !== 1) return;

        event.preventDefault();

        if (middleClick === "singleClickPaste") {
          setSelectedId(item.id);
          closePreview("middleClickPaste");
          pasteClipboardItem(item.id, false);
          return;
        }

        if (middleClick === "singleClickPastePlain") {
          setSelectedId(item.id);
          closePreview("middleClickPastePlain");
          pasteClipboardItem(item.id, true);
          return;
        }

        if (middleClick === "singleClickCopy") {
          setSelectedId(item.id);
          closePreview("middleClickCopy");
          writeToClipboard(item.id, false);
          return;
        }

        if (middleClick === "singleClickCopyPlain") {
          setSelectedId(item.id);
          closePreview("middleClickCopyPlain");
          writeToClipboard(item.id, true);
        }

        return;
      }

      setSelectedId(item.id);

      if (autoPaste === "singleClickPaste") {
        closePreview("singleClickPaste");
        pasteClipboardItem(item.id, false);
        return;
      }

      if (autoPaste === "singleClickCopy") {
        closePreview("singleClickCopy");
        writeToClipboard(item.id, false);
      }
    };

    const handleAuxClick = (event: ReactMouseEvent<HTMLDivElement>) => {
      if (event.button !== 1) return;

      event.preventDefault();
    };

    const handleDoubleClick = () => {
      if (autoPaste === "doubleClickPaste") {
        closePreview("doubleClickPaste");
        pasteClipboardItem(item.id, false);
        return;
      }

      if (autoPaste === "doubleClickCopy") {
        closePreview("doubleClickCopy");
        writeToClipboard(item.id, false);
      }
    };

    const availableActions = getAllowedClipboardActions(
      item.availableActions,
      item,
      canDeleteItem,
    );
    const visibleQuickActions = getAllowedItemActions(
      quickActions,
      item,
      canDeleteItem,
    );

    return (
      <div className={cn("px-3", { "pt-3": index !== 0 })}>
        <ClipboardCard
          availableActions={availableActions}
          hintKey={hintKey}
          isLinkActive={isModifierPressed}
          isSelected={
            selectedId === null
              ? index === firstVisibleIndex
              : item.id === selectedId
          }
          item={item}
          onAuxClick={handleAuxClick}
          onDoubleClick={handleDoubleClick}
          onMouseDown={handleMouseDown}
          onOpenLink={handleOpenLink}
          onPointerEnter={handlePointerEnter}
          onPointerLeave={handlePointerLeave}
          onPointerMove={handlePointerMove}
          onQuickAction={handleQuickAction}
          onQuickPaste={hintKey ? handleQuickPaste : void 0}
          quickActionLabels={quickActionLabels}
          quickActions={visibleQuickActions}
          rootRef={registerItemElement(item.id)}
          showOriginalOnHover={showOriginalPreview}
        />
      </div>
    );
  }

  function renderPlaceholderItem(index: number) {
    return (
      <div aria-hidden="true" className={cn("px-3", { "pt-3": index !== 0 })}>
        <div className="min-h-24 rounded-2 border border-ant-border-secondary bg-ant-fill-quaternary p-2">
          <div className="flex items-center gap-1 text-ant-secondary text-xs">
            <span className="size-4 rounded-1 bg-ant-fill-secondary" />
            <span className="h-3 w-16 rounded-1 bg-ant-fill-secondary" />
          </div>
          <div className="mt-3 flex flex-col gap-2">
            <span className="h-3 w-9/12 rounded-1 bg-ant-fill-secondary" />
            <span className="h-3 w-6/12 rounded-1 bg-ant-fill-secondary" />
          </div>
        </div>
      </div>
    );
  }

  function getNextKeyboardTarget(event: KeyboardEvent) {
    const nextIndex = getNextKeyboardIndex(
      getItemIndexById,
      firstVisibleIndex,
      selectedId,
      total,
      event.key,
    );
    const item = getItem(nextIndex);
    if (!item) {
      loadRange(nextIndex, nextIndex);
      return null;
    }

    return { index: nextIndex, item };
  }

  function canDeleteItem(item: ClipboardItem) {
    if (item.isPinned && !deletePinnedItems) return false;

    if (!item.isFavorite) return true;

    if (!deleteFavoriteItems) return false;

    if (!deleteFavoriteItemsOnlyInFavoriteGroup) return true;

    return range === "favorite";
  }

  function closeTopEscapeLayer() {
    if (previewSession !== null) {
      closePreview("escape");
      return;
    }

    if (clipboardViewState.groupId !== null) {
      clipboardViewState.groupId = null;
      return;
    }

    if (clipboardViewState.category !== null) {
      clipboardViewState.category = null;
      return;
    }

    void hideWindow(WINDOW_LABEL.CLIPBOARD);
  }
};

function getEmptyDescription(
  t: TFunction<"clipboard">,
  keyword: string,
  range: ClipboardRange,
  category: ClipboardKind | null,
  groupId: string | null,
  groupName: string | null,
) {
  const isSearching = keyword.length > 0;
  const isFavorite = range === "favorite";
  const hasGroup = groupId !== null;
  const categoryLabel = category ? t(`empty.categories.${category}`) : "";
  const groupLabel = groupName ?? t("empty.groupFallback");

  if (isSearching) {
    return getSearchingEmptyDescription(
      t,
      keyword,
      isFavorite,
      hasGroup,
      groupLabel,
      categoryLabel,
    );
  }

  if (hasGroup) {
    if (isFavorite && category) {
      return t("empty.groupFavoriteCategory", {
        category: categoryLabel,
        group: groupLabel,
      });
    }

    if (isFavorite) {
      return t("empty.groupFavorites", { group: groupLabel });
    }

    if (category) {
      return t("empty.groupCategory", {
        category: categoryLabel,
        group: groupLabel,
      });
    }

    return t("empty.group", { group: groupLabel });
  }

  if (isFavorite && category) {
    return t("empty.favoriteCategory", { category: categoryLabel });
  }

  if (category) {
    return t("empty.category", { category: categoryLabel });
  }

  return t(isFavorite ? "empty.favorites" : "empty.history");
}

function getSearchingEmptyDescription(
  t: TFunction<"clipboard">,
  keyword: string,
  isFavorite: boolean,
  hasGroup: boolean,
  groupLabel: string,
  categoryLabel: string,
) {
  const hasCategory = categoryLabel.length > 0;

  if (hasGroup) {
    if (isFavorite && hasCategory) {
      return t("empty.searchGroupFavoriteCategory", {
        category: categoryLabel,
        group: groupLabel,
        keyword,
      });
    }

    if (isFavorite) {
      return t("empty.searchGroupFavorites", {
        group: groupLabel,
        keyword,
      });
    }

    if (hasCategory) {
      return t("empty.searchGroupCategory", {
        category: categoryLabel,
        group: groupLabel,
        keyword,
      });
    }

    return t("empty.searchGroup", { group: groupLabel, keyword });
  }

  if (isFavorite && hasCategory) {
    return t("empty.searchFavoriteCategory", {
      category: categoryLabel,
      keyword,
    });
  }

  if (isFavorite) {
    return t("empty.searchFavorites", { keyword });
  }

  if (hasCategory) {
    return t("empty.searchCategory", { category: categoryLabel, keyword });
  }

  return t("empty.searchHistory", { keyword });
}

function getCurrentGroupName(
  groups: ClipboardGroupRecord[],
  groupId: string | null,
) {
  if (!groupId) return null;

  const current = groups.find((record) => {
    return record.id === groupId;
  });

  return current?.name ?? null;
}

function getNextKeyboardIndex(
  getItemIndexById: (id: string) => number | null,
  firstVisibleIndex: number,
  selectedId: string | null,
  total: number,
  key: string,
) {
  const selectedIndex =
    selectedId === null ? null : getItemIndexById(selectedId);
  const currentIndex = selectedIndex ?? firstVisibleIndex;

  if (key === "ArrowUp") {
    return Math.max(0, currentIndex - 1);
  }

  return Math.min(total - 1, currentIndex + 1);
}

function getAllowedClipboardActions(
  actions: ClipboardAction[] | undefined,
  item: ClipboardItem,
  canDeleteItem: (item: ClipboardItem) => boolean,
) {
  if (canDeleteItem(item)) return actions;

  return actions?.filter((action) => {
    return action !== "delete";
  });
}

function getOpenClipboardAction(actions: ClipboardAction[] | undefined) {
  const openActions: ClipboardAction[] = [
    "openLink",
    "sendEmail",
    "revealInFinder",
    "revealInExplorer",
  ];

  return actions?.find((action) => {
    return openActions.includes(action);
  });
}

function getAllowedItemActions(
  actions: readonly ItemAction[],
  item: ClipboardItem,
  canDeleteItem: (item: ClipboardItem) => boolean,
) {
  if (canDeleteItem(item)) return [...actions];

  return actions.filter((action) => {
    return action !== "delete";
  });
}

function getSelectedIdAfterDelete(
  getItem: (index: number) => ClipboardItem | null,
  getItemIndexById: (id: string) => number | null,
  activeFallbackIndex: number,
  selectedId: string | null,
  deletedId: string,
) {
  const deletedIndex = getItemIndexById(deletedId);
  if (deletedIndex === null) return selectedId;

  const activeId =
    selectedId ?? getItem(activeFallbackIndex)?.id ?? getItem(0)?.id ?? null;

  if (activeId !== deletedId) return selectedId;

  const nextItem = getItem(deletedIndex + 1) ?? getItem(deletedIndex - 1);

  return nextItem?.id ?? null;
}

function shouldUseNativeCopy(event: KeyboardEvent) {
  const target = event.target;
  if (target instanceof HTMLElement) {
    const tagName = target.tagName.toLowerCase();
    if (target.isContentEditable) return true;
    if (tagName === "input" || tagName === "textarea") return true;
  }

  const selection = window.getSelection();

  return Boolean(selection && !selection.isCollapsed);
}

const computeItemKey = (index: number, item?: ClipboardItem) => {
  return item?.id ?? `placeholder-${index}`;
};

const TopItemList: FC<TopItemListProps> = (props) => {
  const { children, style } = props;

  return (
    <div className="relative z-10 bg-ant-container" style={style}>
      {children}
    </div>
  );
};

function countLeadingPinnedItems(
  getItem: (index: number) => ClipboardItem | null,
) {
  let count = 0;

  while (true) {
    const item = getItem(count);
    if (!item?.isPinned) break;

    count += 1;
  }

  return count;
}

function shouldRefreshCurrentGroup(
  range: ClipboardRange,
  category: ClipboardKind | null,
  groupId: string | null,
  kind?: ClipboardKind,
) {
  if (groupId) return false;
  if (range === "favorite") return false;
  if (!category) return true;
  if (kind === void 0) return false;

  return category === kind;
}

export default List;
