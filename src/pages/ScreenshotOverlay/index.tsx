import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEventListener, useMemoizedFn, useMount, useUnmount } from "ahooks";
import type { FC, PointerEvent as ReactPointerEvent } from "react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  cancelScreenshotCapture,
  commitScreenshotSelection,
  getScreenshotOverlayFrame,
  getScreenshotOverlayState,
  notifyScreenshotOverlayReady,
  type ScreenshotOverlayState,
} from "@/commands";
import { TAURI_EVENT } from "@/constants/events";
import { cn } from "@/utils/cn";
import { isMac } from "@/utils/is";
import { log } from "@/utils/log";
import {
  type PixelPoint,
  type PixelRect,
  rectContainsPoint,
  rectFromEdges,
  snapEdgeInside,
} from "@/utils/pixelRect";
import { drawOverlay } from "./drawOverlay";
import {
  createOverlaySession,
  isCurrentLoad,
  type OverlaySelectionMode,
  type OverlaySessionCommand,
  type OverlaySessionEvent,
  overlaySessionReducer,
} from "./model/overlaySession";

interface OverlaySessionPayload {
  label: string;
  sessionId: number | null;
}

type SelectionMode = OverlaySelectionMode;

const HINT_DURATION_MS = 2600;
const MIN_SELECTION = 2;

/** Frozen full-monitor frame used only to pick an area or window. */
const ScreenshotOverlay: FC = () => {
  const { t } = useTranslation("screenshot");
  const label = getCurrentWebviewWindow().label;
  const frameCanvasRef = useRef<HTMLCanvasElement>(null);
  const marksCanvasRef = useRef<HTMLCanvasElement>(null);
  const frameRef = useRef<ImageBitmap | null>(null);
  const sessionModelRef = useRef(createOverlaySession());
  const mountedRef = useRef(true);
  const unlistenRef = useRef<UnlistenFn | null>(null);
  const pointerRef = useRef<PixelPoint | null>(null);
  const dragOriginRef = useRef<PixelPoint | null>(null);
  const selectionRef = useRef<PixelRect | null>(null);
  const modeRef = useRef<SelectionMode>("area");
  const drawFrameRef = useRef(0);
  const [session, setSession] = useState<ScreenshotOverlayState | null>(null);
  const [mode, setMode] = useState<SelectionMode>("area");
  const [hintVisible, setHintVisible] = useState(false);

  modeRef.current = mode;

  const dispatch = (event: OverlaySessionEvent) => {
    const step = overlaySessionReducer(sessionModelRef.current, event);
    sessionModelRef.current = step.session;

    return step.commands;
  };

  const runCommands = (commands: OverlaySessionCommand[]) => {
    for (const command of commands) {
      switch (command.type) {
        case "release": {
          releaseView();
          break;
        }
        case "startSession": {
          pointerRef.current = null;
          dragOriginRef.current = null;
          selectionRef.current = null;
          setMode(command.mode);
          setHintVisible(true);
          break;
        }
        case "sendCommit": {
          void sendCommit(command.sessionId, command.rect);
          break;
        }
        case "sendCancel": {
          void sendCancel(command.sessionId);
          break;
        }
      }
    }
  };

  const releaseView = () => {
    frameRef.current?.close();
    frameRef.current = null;
    pointerRef.current = null;
    dragOriginRef.current = null;
    selectionRef.current = null;
    releaseCanvases();
    setSession(null);
  };

  // Overlays stay alive between captures, so drop the full-monitor backing stores.
  const releaseCanvases = () => {
    for (const canvas of [frameCanvasRef.current, marksCanvasRef.current]) {
      if (!canvas) continue;

      canvas.width = 0;
      canvas.height = 0;
    }
  };

  const loadSession = async () => {
    dispatch({ type: "loadStarted" });
    const generation = sessionModelRef.current.generation;
    const startedAt = performance.now();
    let requestedState: ScreenshotOverlayState | null = null;

    try {
      const state = await getScreenshotOverlayState(label);
      requestedState = state;
      const stateMs = performance.now() - startedAt;
      if (!isCurrentLoad(sessionModelRef.current, generation)) return;
      if (!state) {
        runCommands(
          dispatch({ generation, snapshot: null, type: "stateLoaded" }),
        );
        return;
      }

      if (state.ready) {
        const transferStartedAt = performance.now();
        const buffer = await getScreenshotOverlayFrame(label, state.sessionId);
        if (!isCurrentLoad(sessionModelRef.current, generation)) return;
        const ipcMs = performance.now() - transferStartedAt;
        const decodeStartedAt = performance.now();
        const bitmap = await createImageBitmap(
          new Blob([buffer], { type: "image/png" }),
        );
        if (!isCurrentLoad(sessionModelRef.current, generation)) {
          bitmap.close();
          return;
        }
        if (bitmap.width !== state.width || bitmap.height !== state.height) {
          bitmap.close();
          throw new Error(
            "screenshot overlay PNG dimensions do not match the monitor",
          );
        }

        frameRef.current?.close();
        frameRef.current = bitmap;
        log.info("screenshot overlay frame ready", {
          decodeMs: Math.round(performance.now() - decodeStartedAt),
          encodedBytes: buffer.byteLength,
          ipcMs: Math.round(ipcMs),
          label,
          stateMs: Math.round(stateMs),
        });
      } else {
        frameRef.current?.close();
        frameRef.current = null;
      }

      const commands = dispatch({
        generation,
        snapshot: state,
        type: "stateLoaded",
      });
      setSession(state);
      runCommands(commands);
    } catch (error) {
      log.error("load screenshot overlay failed", error);
      if (
        isCurrentLoad(sessionModelRef.current, generation) &&
        requestedState?.ready
      ) {
        try {
          await cancelScreenshotCapture(requestedState.sessionId);
        } catch (cancelError) {
          log.error(
            "cancel invalid screenshot overlay frame failed",
            cancelError,
          );
        }
      }
    }
  };

  useMount(async () => {
    try {
      const unlisten = await listen<OverlaySessionPayload>(
        TAURI_EVENT.SCREENSHOT_OVERLAY_SESSION,
        (event) => {
          if (!mountedRef.current || event.payload.label !== label) return;
          if (event.payload.sessionId === null) {
            runCommands(dispatch({ type: "reset" }));
            return;
          }

          void loadSession();
        },
      );
      if (!mountedRef.current) {
        unlisten();
        return;
      }
      unlistenRef.current = unlisten;
      await loadSession();
    } catch (error) {
      log.error("subscribe screenshot overlay failed", error);
    }
  });

  useUnmount(() => {
    mountedRef.current = false;
    dispatch({ type: "unmounted" });
    unlistenRef.current?.();
    cancelAnimationFrame(drawFrameRef.current);
    frameRef.current?.close();
    frameRef.current = null;
  });

  const revealWhenPainted = useMemoizedFn(async (sessionId: number) => {
    try {
      await notifyScreenshotOverlayReady(label, sessionId);
    } catch (error) {
      log.error("reveal screenshot overlay failed", error);
    }
  });

  const cancelAfterPaintFailure = useMemoizedFn(async (sessionId: number) => {
    try {
      await cancelScreenshotCapture(sessionId);
    } catch (error) {
      log.error("cancel screenshot overlay after paint failure failed", error);
    }
  });

  const hoveredWindow = (point: PixelPoint | null) => {
    if (!session || !point) return null;

    return (
      session.windows.find((rect) => {
        return rectContainsPoint(rect, point);
      }) ?? null
    );
  };

  const currentHighlight = () => {
    if (modeRef.current === "window") return hoveredWindow(pointerRef.current);

    return selectionRef.current;
  };

  const drawMarks = useMemoizedFn(() => {
    const marks = marksCanvasRef.current;
    const context = marks?.getContext("2d");
    const frame = frameCanvasRef.current;
    if (!marks || !context || !frame || !session) return;

    drawOverlay(context, {
      bitmap: frame,
      highlight: currentHighlight(),
      pointer: pointerRef.current,
      ratio: session.width / Math.max(1, marks.clientWidth),
      showCrosshair:
        modeRef.current === "area" && dragOriginRef.current === null,
      showLoupe: session.ready,
    });
  });

  useLayoutEffect(() => {
    const canvas = frameCanvasRef.current;
    const marks = marksCanvasRef.current;
    const frame = frameRef.current;
    if (!session || !canvas || !marks) return;

    const startedAt = performance.now();
    if (canvas.width !== session.width || canvas.height !== session.height) {
      canvas.width = session.width;
      canvas.height = session.height;
      marks.width = session.width;
      marks.height = session.height;
    }
    if (frame) {
      const context = canvas.getContext("2d");
      if (!context) {
        frame.close();
        frameRef.current = null;
        log.error("screenshot overlay canvas context is unavailable");
        void cancelAfterPaintFailure(session.sessionId);
        return;
      }
      try {
        context.drawImage(frame, 0, 0);
      } catch (error) {
        log.error("paint screenshot overlay PNG failed", error);
        void cancelAfterPaintFailure(session.sessionId);
        return;
      } finally {
        frame.close();
        frameRef.current = null;
      }
    }
    drawMarks();
    log.info("screenshot overlay painted", {
      label,
      paintMs: Math.round(performance.now() - startedAt),
    });

    // Animation frames can pause while the overlay is hidden, so reveal right after drawing.
    void revealWhenPainted(session.sessionId);
  }, [cancelAfterPaintFailure, drawMarks, revealWhenPainted, session, label]);

  useEffect(() => {
    if (!hintVisible) return;

    const timer = window.setTimeout(() => {
      setHintVisible(false);
    }, HINT_DURATION_MS);

    return () => {
      window.clearTimeout(timer);
    };
  }, [hintVisible]);

  const scheduleDraw = () => {
    cancelAnimationFrame(drawFrameRef.current);
    drawFrameRef.current = requestAnimationFrame(drawMarks);
  };

  useEffect(() => {
    scheduleDraw();
  });

  const toFramePoint = (event: ReactPointerEvent<HTMLCanvasElement>) => {
    const bounds = event.currentTarget.getBoundingClientRect();
    const scaleX = (session?.width ?? 0) / Math.max(1, bounds.width);
    const scaleY = (session?.height ?? 0) / Math.max(1, bounds.height);

    return {
      x: (event.clientX - bounds.left) * scaleX,
      y: (event.clientY - bounds.top) * scaleY,
    };
  };

  const commit = (rect: PixelRect) => {
    runCommands(dispatch({ rect, type: "commitRequested" }));
  };

  const sendCommit = async (sessionId: number, rect: PixelRect) => {
    try {
      await commitScreenshotSelection(label, sessionId, rect);
    } catch (error) {
      dispatch({ type: "commitFailed" });
      log.error("commit screenshot selection failed", error);
    }
  };

  const cancel = () => {
    runCommands(dispatch({ type: "cancelRequested" }));
  };

  const sendCancel = async (sessionId: number) => {
    try {
      await cancelScreenshotCapture(sessionId);
    } catch (error) {
      dispatch({ type: "cancelFailed" });
      log.error("cancel screenshot capture failed", error);
    }
  };

  const handlePointerDown = (event: ReactPointerEvent<HTMLCanvasElement>) => {
    if (!session) return;

    if (event.button === 2) {
      cancel();
      return;
    }

    if (event.button !== 0) return;

    event.currentTarget.setPointerCapture(event.pointerId);
    const point = toFramePoint(event);
    pointerRef.current = point;

    if (modeRef.current === "area") {
      dragOriginRef.current = snapEdgeInside(point, bounds(session));
      selectionRef.current = null;
    }

    setHintVisible(false);
    scheduleDraw();
  };

  const handlePointerMove = (event: ReactPointerEvent<HTMLCanvasElement>) => {
    if (!session) return;

    const point = toFramePoint(event);
    pointerRef.current = point;

    const origin = dragOriginRef.current;
    if (origin) {
      selectionRef.current = rectFromEdges(
        origin,
        snapEdgeInside(point, bounds(session)),
      );
    }

    scheduleDraw();
  };

  const handlePointerUp = (event: ReactPointerEvent<HTMLCanvasElement>) => {
    if (!session || event.button !== 0) return;

    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }

    if (modeRef.current === "window") {
      const target = hoveredWindow(toFramePoint(event));
      if (target) commit(target);
      return;
    }

    const selection = selectionRef.current;
    dragOriginRef.current = null;

    if (
      selection &&
      selection.width >= MIN_SELECTION &&
      selection.height >= MIN_SELECTION
    ) {
      commit(selection);
      return;
    }

    selectionRef.current = null;
    scheduleDraw();
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    if (!session) return;

    if (event.key === "Escape") {
      event.preventDefault();
      cancel();
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      commit(bounds(session));
      return;
    }

    if (event.code === "Space") {
      event.preventDefault();
      dragOriginRef.current = null;
      selectionRef.current = null;
      setMode((current) => {
        return current === "area" ? "window" : "area";
      });
      setHintVisible(true);
    }
  };

  const handleContextMenu = (event: MouseEvent) => {
    event.preventDefault();
  };

  useEventListener("keydown", handleKeyDown);
  useEventListener("contextmenu", handleContextMenu);

  return (
    <div
      className={cn("fixed inset-0 select-none overflow-hidden", {
        "bg-black": !isMac,
      })}
    >
      <canvas
        className={cn("absolute inset-0 block h-full w-full", {
          invisible: !session,
        })}
        ref={frameCanvasRef}
      />
      <canvas
        className={cn("absolute inset-0 block h-full w-full touch-none", {
          "cursor-crosshair": mode === "area",
          "cursor-default": mode === "window",
          invisible: !session,
        })}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        ref={marksCanvasRef}
      />

      {session && (
        <div
          className={cn(
            "pointer-events-none absolute top-6 left-1/2 -translate-x-1/2 rounded-full bg-black/70 px-4 py-2 text-sm text-white shadow-lg transition-opacity duration-500",
            { "opacity-0": !hintVisible },
          )}
        >
          {t(mode === "area" ? "overlay.areaHint" : "overlay.windowHint")}
        </div>
      )}
    </div>
  );
};

function bounds(session: ScreenshotOverlayState): PixelRect {
  return { height: session.height, width: session.width, x: 0, y: 0 };
}

export default ScreenshotOverlay;
