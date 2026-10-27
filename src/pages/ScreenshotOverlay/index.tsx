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
import { useTauriListen } from "@/hooks/useTauriListen";
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

interface OverlaySessionPayload {
  label: string;
  sessionId: number | null;
}

type SelectionMode = "area" | "window";

const HINT_DURATION_MS = 2600;
const MIN_SELECTION = 2;

/** Frozen full-monitor frame used only to pick an area or window. */
const ScreenshotOverlay: FC = () => {
  const { t } = useTranslation("screenshot");
  const label = getCurrentWebviewWindow().label;
  const frameCanvasRef = useRef<HTMLCanvasElement>(null);
  const marksCanvasRef = useRef<HTMLCanvasElement>(null);
  const frameRef = useRef<ImageData | null>(null);
  const loadGenerationRef = useRef(0);
  const pointerRef = useRef<PixelPoint | null>(null);
  const dragOriginRef = useRef<PixelPoint | null>(null);
  const selectionRef = useRef<PixelRect | null>(null);
  const modeRef = useRef<SelectionMode>("area");
  const drawFrameRef = useRef(0);
  const committedRef = useRef(false);
  const sessionIdRef = useRef<number | null>(null);
  const pendingCommitRef = useRef<PixelRect | null>(null);
  const [session, setSession] = useState<ScreenshotOverlayState | null>(null);
  const [mode, setMode] = useState<SelectionMode>("area");
  const [hintVisible, setHintVisible] = useState(false);

  modeRef.current = mode;

  const reset = () => {
    loadGenerationRef.current += 1;
    frameRef.current = null;
    sessionIdRef.current = null;
    pendingCommitRef.current = null;
    pointerRef.current = null;
    dragOriginRef.current = null;
    selectionRef.current = null;
    committedRef.current = false;
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
    const generation = loadGenerationRef.current + 1;
    loadGenerationRef.current = generation;
    const startedAt = performance.now();

    try {
      const state = await getScreenshotOverlayState(label);
      const stateMs = performance.now() - startedAt;
      if (generation !== loadGenerationRef.current) return;
      if (!state) {
        reset();
        return;
      }

      if (state.ready) {
        const buffer = await getScreenshotOverlayFrame(label, state.sessionId);
        if (generation !== loadGenerationRef.current) {
          return;
        }

        frameRef.current = new ImageData(
          new Uint8ClampedArray(buffer),
          state.width,
          state.height,
        );
        log.info("screenshot overlay frame ready", {
          frameMs: Math.round(performance.now() - startedAt - stateMs),
          label,
          stateMs: Math.round(stateMs),
        });
      }

      // The pixels of an overlay shown early arrive as a second load of the
      // same session; keep what the user has selected so far.
      if (sessionIdRef.current !== state.sessionId) {
        sessionIdRef.current = state.sessionId;
        pendingCommitRef.current = null;
        pointerRef.current = null;
        dragOriginRef.current = null;
        selectionRef.current = null;
        committedRef.current = false;
        setMode(state.mode === "window" ? "window" : "area");
        setHintVisible(true);
      }
      setSession(state);

      const pending = pendingCommitRef.current;
      if (state.ready && pending) {
        pendingCommitRef.current = null;
        void sendCommit(state.sessionId, pending);
      }
    } catch (error) {
      log.error("load screenshot overlay failed", error);
    }
  };

  useMount(() => {
    void loadSession();
  });

  useUnmount(() => {
    cancelAnimationFrame(drawFrameRef.current);
    frameRef.current = null;
  });

  useTauriListen<OverlaySessionPayload>(
    TAURI_EVENT.SCREENSHOT_OVERLAY_SESSION,
    (event) => {
      if (event.payload.label !== label) return;

      if (event.payload.sessionId === null) {
        reset();
        return;
      }

      void loadSession();
    },
  );

  const revealWhenPainted = useMemoizedFn(async (sessionId: number) => {
    try {
      await notifyScreenshotOverlayReady(label, sessionId);
    } catch (error) {
      log.error("reveal screenshot overlay failed", error);
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
      canvas.getContext("2d")?.putImageData(frame, 0, 0);
      frameRef.current = null;
    }
    drawMarks();
    log.info("screenshot overlay painted", {
      label,
      paintMs: Math.round(performance.now() - startedAt),
    });

    // Animation frames can pause while the overlay is hidden, so reveal right after drawing.
    void revealWhenPainted(session.sessionId);
  }, [drawMarks, revealWhenPainted, session, label]);

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

  const commit = async (rect: PixelRect) => {
    if (!session || committedRef.current) return;

    committedRef.current = true;

    // A selection made before the pixels arrive is sent when they do.
    if (!session.ready) {
      pendingCommitRef.current = rect;
      return;
    }

    await sendCommit(session.sessionId, rect);
  };

  const sendCommit = async (sessionId: number, rect: PixelRect) => {
    try {
      await commitScreenshotSelection(label, sessionId, rect);
    } catch (error) {
      committedRef.current = false;
      log.error("commit screenshot selection failed", error);
    }
  };

  const cancel = async () => {
    if (!session) return;
    if (committedRef.current && pendingCommitRef.current === null) return;

    committedRef.current = true;
    pendingCommitRef.current = null;

    try {
      await cancelScreenshotCapture(session.sessionId);
    } catch (error) {
      committedRef.current = false;
      log.error("cancel screenshot capture failed", error);
    }
  };

  const handlePointerDown = (event: ReactPointerEvent<HTMLCanvasElement>) => {
    if (!session) return;

    if (event.button === 2) {
      void cancel();
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
      if (target) void commit(target);
      return;
    }

    const selection = selectionRef.current;
    dragOriginRef.current = null;

    if (
      selection &&
      selection.width >= MIN_SELECTION &&
      selection.height >= MIN_SELECTION
    ) {
      void commit(selection);
      return;
    }

    selectionRef.current = null;
    scheduleDraw();
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    if (!session) return;

    if (event.key === "Escape") {
      event.preventDefault();
      void cancel();
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      void commit(bounds(session));
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
