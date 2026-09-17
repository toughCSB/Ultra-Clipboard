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
  const bitmapRef = useRef<ImageBitmap | null>(null);
  const loadGenerationRef = useRef(0);
  const pointerRef = useRef<PixelPoint | null>(null);
  const dragOriginRef = useRef<PixelPoint | null>(null);
  const selectionRef = useRef<PixelRect | null>(null);
  const modeRef = useRef<SelectionMode>("area");
  const drawFrameRef = useRef(0);
  const committedRef = useRef(false);
  const [session, setSession] = useState<ScreenshotOverlayState | null>(null);
  const [mode, setMode] = useState<SelectionMode>("area");
  const [hintVisible, setHintVisible] = useState(false);

  modeRef.current = mode;

  const reset = () => {
    loadGenerationRef.current += 1;
    bitmapRef.current?.close();
    bitmapRef.current = null;
    pointerRef.current = null;
    dragOriginRef.current = null;
    selectionRef.current = null;
    committedRef.current = false;
    setSession(null);
  };

  const loadSession = async () => {
    const generation = loadGenerationRef.current + 1;
    loadGenerationRef.current = generation;

    try {
      const state = await getScreenshotOverlayState(label);
      if (generation !== loadGenerationRef.current) return;
      if (!state) {
        reset();
        return;
      }

      const buffer = await getScreenshotOverlayFrame(label, state.sessionId);
      const bitmap = await createImageBitmap(
        new ImageData(new Uint8ClampedArray(buffer), state.width, state.height),
      );
      if (generation !== loadGenerationRef.current) {
        bitmap.close();
        return;
      }

      bitmapRef.current?.close();
      bitmapRef.current = bitmap;
      pointerRef.current = null;
      dragOriginRef.current = null;
      selectionRef.current = null;
      committedRef.current = false;
      setMode(state.mode === "window" ? "window" : "area");
      setHintVisible(true);
      setSession(state);
    } catch (error) {
      log.error("load screenshot overlay failed", error);
    }
  };

  useMount(() => {
    void loadSession();
  });

  useUnmount(() => {
    cancelAnimationFrame(drawFrameRef.current);
    bitmapRef.current?.close();
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
    const bitmap = bitmapRef.current;
    if (!marks || !context || !bitmap || !session) return;

    drawOverlay(context, {
      bitmap,
      highlight: currentHighlight(),
      pointer: pointerRef.current,
      ratio: session.width / Math.max(1, marks.clientWidth),
      showCrosshair:
        modeRef.current === "area" && dragOriginRef.current === null,
    });
  });

  useLayoutEffect(() => {
    const canvas = frameCanvasRef.current;
    const marks = marksCanvasRef.current;
    const bitmap = bitmapRef.current;
    if (!session || !canvas || !marks || !bitmap) return;

    canvas.width = session.width;
    canvas.height = session.height;
    marks.width = session.width;
    marks.height = session.height;
    canvas.getContext("2d")?.drawImage(bitmap, 0, 0);
    drawMarks();

    // Animation frames can pause while the overlay is hidden, so reveal right after drawing.
    void revealWhenPainted(session.sessionId);
  }, [drawMarks, revealWhenPainted, session]);

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

    try {
      await commitScreenshotSelection(label, session.sessionId, rect);
    } catch (error) {
      committedRef.current = false;
      log.error("commit screenshot selection failed", error);
    }
  };

  const cancel = async () => {
    if (!session || committedRef.current) return;

    committedRef.current = true;

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
    <div className="fixed inset-0 select-none overflow-hidden bg-black">
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
