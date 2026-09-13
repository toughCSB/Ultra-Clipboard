import { useEventListener } from "ahooks";
import type { PointerEvent as ReactPointerEvent } from "react";
import { useEffect, useRef, useState } from "react";
import { useSnapshot } from "valtio";
import { showClipboardPreview } from "@/commands";
import { TAURI_EVENT } from "@/constants/events";
import { WINDOW_LABEL } from "@/constants/windows";
import { useKeyboardEvent } from "@/hooks/useKeyboardEvent";
import { useTauriListen } from "@/hooks/useTauriListen";
import { settingsState } from "@/stores/settings";
import type { ClipboardItem } from "@/types/clipboard";
import { log } from "@/utils/log";
import {
  clearHoverTimer,
  closeClipboardPreviewSilently,
  HOVER_DELAY_MS,
  HOVER_HIDE_BUFFER_MS,
  isSpaceKey,
  type PreviewSession,
  type PreviewTrigger,
  type UseClipboardPreviewControllerOptions,
  type WindowVisibilityPayload,
} from "./previewController";

const KEYBOARD_PREVIEW_MAX_FRAMES = 36;
const KEYBOARD_PREVIEW_STABLE_FRAMES = 2;
const KEYBOARD_PREVIEW_RECT_EPSILON = 0.5;

interface PreviewRectSnapshot {
  left: number;
  top: number;
  width: number;
  height: number;
}

interface KeyboardPreviewTarget {
  frames: number;
  item: ClipboardItem;
  lastRect: PreviewRectSnapshot | null;
  stableFrames: number;
}

/** Manage hover and keyboard preview lifecycles, including hidden-window cleanup. */
export function useClipboardPreviewController(
  options: UseClipboardPreviewControllerOptions,
) {
  const { getActiveItem, itemElementMapRef, onHoverSelect } = options;
  const [previewSession, setPreviewSession] = useState<PreviewSession | null>(
    null,
  );
  const hoverTimerRef = useRef<number | null>(null);
  const hoverHideTimerRef = useRef<number | null>(null);
  const clipboardWindowVisibleRef = useRef(true);
  const previewSessionRef = useRef<PreviewSession | null>(null);
  const previewOpenRequestIdRef = useRef(0);
  const previewMoveFrameRef = useRef<number | null>(null);
  const previewMoveTargetRef = useRef<{
    item: ClipboardItem;
    pointerY: number;
  } | null>(null);
  const keyboardPreviewFrameRef = useRef<number | null>(null);
  const keyboardPreviewTargetRef = useRef<KeyboardPreviewTarget | null>(null);
  const pendingHoverTargetRef = useRef<{
    item: ClipboardItem;
    pointerY: number;
  } | null>(null);
  const settingsSnapshot = useSnapshot(settingsState);
  const previewSettings = settingsSnapshot.clipboard.preview;

  useEffect(() => {
    if (!previewSettings.hoverEnabled && previewSession?.trigger === "hover") {
      clearHoverTimer(hoverTimerRef);
      clearHoverTimer(hoverHideTimerRef);
      previewSessionRef.current = null;
      setPreviewSession(null);
      closeClipboardPreviewSilently("hoverDisabled");
      return;
    }

    if (
      !previewSettings.spaceEnabled &&
      previewSession?.trigger === "keyboard"
    ) {
      clearHoverTimer(hoverTimerRef);
      clearHoverTimer(hoverHideTimerRef);
      previewSessionRef.current = null;
      setPreviewSession(null);
      closeClipboardPreviewSilently("spaceDisabled");
    }
  }, [
    previewSettings.hoverEnabled,
    previewSettings.spaceEnabled,
    previewSession?.trigger,
  ]);

  useEffect(() => {
    return () => {
      clearHoverTimer(hoverTimerRef);
      clearHoverTimer(hoverHideTimerRef);
      pendingHoverTargetRef.current = null;
      if (keyboardPreviewFrameRef.current !== null) {
        window.cancelAnimationFrame(keyboardPreviewFrameRef.current);
        keyboardPreviewFrameRef.current = null;
      }
      keyboardPreviewTargetRef.current = null;
      if (previewMoveFrameRef.current !== null) {
        window.cancelAnimationFrame(previewMoveFrameRef.current);
        previewMoveFrameRef.current = null;
        previewMoveTargetRef.current = null;
      }
    };
  }, []);

  const handleWindowVisibility = (event: {
    payload: WindowVisibilityPayload;
  }) => {
    if (event.payload.label !== WINDOW_LABEL.CLIPBOARD) return;

    clipboardWindowVisibleRef.current = event.payload.visible;
    if (event.payload.visible) return;

    closePreview("windowHidden");
  };

  useTauriListen(TAURI_EVENT.WINDOW_VISIBILITY, handleWindowVisibility);

  const handleWindowBlur = () => {
    if (!previewSessionRef.current) return;

    closePreview("windowBlur");
  };

  useEventListener("blur", handleWindowBlur, { target: window });

  const handleWindowResize = () => {
    closePreview("windowResize");
  };

  useEventListener("resize", handleWindowResize, { target: window });

  const closePreview = (reason: string) => {
    previewOpenRequestIdRef.current += 1;
    cancelHoverPreview();
    cancelHoverHide();
    cancelPreviewMoveFrame();
    cancelKeyboardPreviewFrame();
    commitPreviewSession(null);
    closeClipboardPreviewSilently(reason);
  };

  const handleItemPointerEnter = (
    item: ClipboardItem,
    event: ReactPointerEvent<HTMLDivElement>,
  ) => {
    const pointerY = event.clientY;

    onHoverSelect(item.id);

    if (!clipboardWindowVisibleRef.current) return;
    if (previewSessionRef.current?.trigger === "keyboard") {
      cancelHoverPreview();
      cancelHoverHide();

      if (previewSessionRef.current.itemId === item.id) {
        cancelKeyboardPreviewFrame();
        return;
      }

      scheduleKeyboardPreviewMove(item);
      return;
    }
    if (!previewSettings.hoverEnabled) return;

    cancelHoverPreview();
    cancelHoverHide();
    pendingHoverTargetRef.current = { item, pointerY };

    if (previewSessionRef.current?.trigger === "hover") {
      if (previewSessionRef.current.itemId === item.id) return;

      void openPreviewForItem(item, "hover", pointerY);
      return;
    }

    hoverTimerRef.current = window.setTimeout(() => {
      hoverTimerRef.current = null;

      if (!clipboardWindowVisibleRef.current) return;
      if (!settingsState.clipboard.preview.hoverEnabled) return;

      const target = pendingHoverTargetRef.current;

      if (!target || target.item.id !== item.id) return;

      void openPreviewForItem(target.item, "hover", target.pointerY);
    }, HOVER_DELAY_MS[previewSettings.hoverDelayMs]);
  };

  const handleItemPointerMove = (
    item: ClipboardItem,
    event: ReactPointerEvent<HTMLDivElement>,
  ) => {
    const pointerY = event.clientY;

    if (hoverTimerRef.current !== null) {
      pendingHoverTargetRef.current = { item, pointerY };
    }

    if (previewSessionRef.current?.trigger !== "hover") return;
    if (previewSessionRef.current.itemId !== item.id) return;

    previewMoveTargetRef.current = {
      item,
      pointerY,
    };

    if (previewMoveFrameRef.current !== null) return;

    previewMoveFrameRef.current = window.requestAnimationFrame(() => {
      previewMoveFrameRef.current = null;

      const target = previewMoveTargetRef.current;
      previewMoveTargetRef.current = null;

      if (!target) return;
      if (previewSessionRef.current?.trigger !== "hover") return;
      if (previewSessionRef.current.itemId !== target.item.id) return;

      void openPreviewForItem(target.item, "hover", target.pointerY);
    });
  };

  const handleItemPointerLeave = () => {
    scheduleHoverHide("itemPointerLeave");
  };

  const handlePreviewAreaPointerLeave = () => {
    scheduleHoverHide("hoverAreaLeave");
  };

  const handleDocumentPointerOut = (event: PointerEvent) => {
    if (event.relatedTarget !== null) return;

    scheduleHoverHide("documentPointerOut");
  };

  useEventListener("pointerout", handleDocumentPointerOut, {
    target: document,
  });

  const handleDocumentPointerCancel = () => {
    scheduleHoverHide("documentPointerCancel");
  };

  useEventListener("pointercancel", handleDocumentPointerCancel, {
    target: document,
  });

  const handlePreviewSpaceDown = (event: KeyboardEvent) => {
    event.preventDefault();

    if (!previewSettings.spaceEnabled) return;
    if (event.repeat && previewSession?.trigger === "keyboard") return;

    cancelHoverPreview();
    cancelHoverHide();

    const activeItem = getActiveItem();

    if (!activeItem) return;

    void openPreviewForItem(activeItem, "keyboard");
  };

  const handlePreviewSpaceUp = (event: KeyboardEvent) => {
    if (!isSpaceKey(event)) return;

    event.preventDefault();

    if (previewSession?.trigger !== "keyboard") return;

    closePreview("spaceUp");
  };

  useKeyboardEvent("keyup", handlePreviewSpaceUp);

  const handleKeyboardPreviewMove = (item: ClipboardItem) => {
    if (previewSessionRef.current?.trigger !== "keyboard") return;

    scheduleKeyboardPreviewMove(item);
  };

  const cancelHoverPreview = () => {
    clearHoverTimer(hoverTimerRef);
    pendingHoverTargetRef.current = null;
  };

  const closeHoverPreviewForScroll = () => {
    if (previewSession?.trigger === "hover") {
      closePreview("scroll");
      return;
    }

    cancelHoverPreview();
  };

  const openPreviewForItem = async (
    item: ClipboardItem,
    trigger: PreviewTrigger,
    pointerY?: number,
  ) => {
    if (!clipboardWindowVisibleRef.current) return;

    const element = itemElementMapRef.current.get(item.id);
    const rect = element?.getBoundingClientRect();

    if (!rect || rect.width <= 0 || rect.height <= 0) return;

    const requestId = previewOpenRequestIdRef.current + 1;
    previewOpenRequestIdRef.current = requestId;
    commitPreviewSession({ itemId: item.id, trigger });

    try {
      if (!clipboardWindowVisibleRef.current) return;

      const state = await showClipboardPreview(item.id, {
        height: rect.height,
        left: rect.left,
        pointerY,
        top: rect.top,
        width: rect.width,
      });

      if (requestId !== previewOpenRequestIdRef.current) return;
      if (!state || !clipboardWindowVisibleRef.current) {
        closePreview("previewShowSuppressed");
      }
    } catch (error) {
      if (requestId === previewOpenRequestIdRef.current) {
        log.error("show clipboard preview failed", { error, trigger });
        commitPreviewSession(null);
      }
      return;
    }
  };

  const cancelHoverHide = () => {
    clearHoverTimer(hoverHideTimerRef);
  };

  const closeHoverPreview = (reason: string) => {
    cancelHoverPreview();
    cancelHoverHide();

    if (previewSessionRef.current?.trigger !== "hover") return;

    closePreview(reason);
  };

  const scheduleHoverHide = (reason: string) => {
    cancelHoverPreview();
    cancelHoverHide();

    if (previewSessionRef.current?.trigger !== "hover") return;

    hoverHideTimerRef.current = window.setTimeout(() => {
      hoverHideTimerRef.current = null;
      closeHoverPreview(reason);
    }, HOVER_HIDE_BUFFER_MS);
  };

  function cancelPreviewMoveFrame() {
    if (previewMoveFrameRef.current === null) return;

    window.cancelAnimationFrame(previewMoveFrameRef.current);
    previewMoveFrameRef.current = null;
    previewMoveTargetRef.current = null;
    pendingHoverTargetRef.current = null;
  }

  function scheduleKeyboardPreviewMove(item: ClipboardItem) {
    cancelKeyboardPreviewFrame();
    keyboardPreviewTargetRef.current = {
      frames: 0,
      item,
      lastRect: null,
      stableFrames: 0,
    };
    requestKeyboardPreviewFrame();
  }

  function requestKeyboardPreviewFrame() {
    keyboardPreviewFrameRef.current = window.requestAnimationFrame(
      handleKeyboardPreviewFrame,
    );
  }

  function handleKeyboardPreviewFrame() {
    keyboardPreviewFrameRef.current = null;

    const target = keyboardPreviewTargetRef.current;

    if (!target) return;

    if (previewSessionRef.current?.trigger !== "keyboard") {
      keyboardPreviewTargetRef.current = null;
      return;
    }

    target.frames += 1;

    const rect = resolveItemRect(target.item.id);

    if (!rect) {
      retryKeyboardPreviewFrame(target);
      return;
    }

    if (
      target.lastRect &&
      isPreviewRectStable(target.lastRect, rect, KEYBOARD_PREVIEW_RECT_EPSILON)
    ) {
      target.stableFrames += 1;
    } else {
      target.stableFrames = 0;
    }

    target.lastRect = rect;

    if (
      target.stableFrames >= KEYBOARD_PREVIEW_STABLE_FRAMES ||
      target.frames >= KEYBOARD_PREVIEW_MAX_FRAMES
    ) {
      keyboardPreviewTargetRef.current = null;
      void openPreviewForItem(target.item, "keyboard");
      return;
    }

    requestKeyboardPreviewFrame();
  }

  function retryKeyboardPreviewFrame(target: KeyboardPreviewTarget) {
    if (target.frames >= KEYBOARD_PREVIEW_MAX_FRAMES) {
      keyboardPreviewTargetRef.current = null;
      return;
    }

    requestKeyboardPreviewFrame();
  }

  function cancelKeyboardPreviewFrame() {
    if (keyboardPreviewFrameRef.current !== null) {
      window.cancelAnimationFrame(keyboardPreviewFrameRef.current);
      keyboardPreviewFrameRef.current = null;
    }

    keyboardPreviewTargetRef.current = null;
  }

  function resolveItemRect(id: string): PreviewRectSnapshot | null {
    const element = itemElementMapRef.current.get(id);
    const rect = element?.getBoundingClientRect();

    if (!rect || rect.width <= 0 || rect.height <= 0) return null;

    return {
      height: rect.height,
      left: rect.left,
      top: rect.top,
      width: rect.width,
    };
  }

  function commitPreviewSession(session: PreviewSession | null) {
    previewSessionRef.current = session;
    setPreviewSession(session);
  }

  return {
    closeHoverPreviewForScroll,
    closePreview,
    handleItemPointerEnter,
    handleItemPointerLeave,
    handleItemPointerMove,
    handleKeyboardPreviewMove,
    handlePreviewAreaPointerLeave,
    handlePreviewSpaceDown,
    previewSession,
  };
}

/** Check whether two DOMRect samples are stable during smooth scrolling. */
function isPreviewRectStable(
  prev: PreviewRectSnapshot,
  next: PreviewRectSnapshot,
  epsilon: number,
) {
  return (
    Math.abs(prev.left - next.left) <= epsilon &&
    Math.abs(prev.top - next.top) <= epsilon &&
    Math.abs(prev.width - next.width) <= epsilon &&
    Math.abs(prev.height - next.height) <= epsilon
  );
}

export { isSpaceKey } from "./previewController";
