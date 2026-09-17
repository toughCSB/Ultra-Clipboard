import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEventListener, useMount, useUnmount } from "ahooks";
import type {
  FC,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
  WheelEvent as ReactWheelEvent,
} from "react";
import { useEffect, useRef, useState } from "react";
import {
  closeScreenshotWindow,
  getScreenshotImage,
  getScreenshotImageInfo,
  notifyScreenshotWindowReady,
  setScreenshotPinScale,
  showScreenshotPinMenu,
} from "@/commands";
import { cn } from "@/utils/cn";
import { log } from "@/utils/log";

const MIN_SCALE = 0.1;
const MAX_SCALE = 4;
const WHEEL_STEP = 1.1;
const DRAG_THRESHOLD_CSS = 3;
const SCALE_BADGE_MS = 900;

/** Always-on-top pinned screenshot that moves by dragging and scales with the wheel. */
const ScreenshotPin: FC = () => {
  const currentWindow = getCurrentWebviewWindow();
  const label = currentWindow.label;
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const bitmapRef = useRef<ImageBitmap | null>(null);
  const scaleRef = useRef(1);
  const pressRef = useRef<{ x: number; y: number } | null>(null);
  const [scalePercent, setScalePercent] = useState<number | null>(null);

  const draw = () => {
    const canvas = canvasRef.current;
    const bitmap = bitmapRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !bitmap || !context) return;

    const ratio = window.devicePixelRatio || 1;
    canvas.width = Math.max(1, Math.round(canvas.clientWidth * ratio));
    canvas.height = Math.max(1, Math.round(canvas.clientHeight * ratio));
    context.imageSmoothingEnabled = canvas.width !== bitmap.width;
    context.imageSmoothingQuality = "high";
    context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  };

  useMount(async () => {
    try {
      const info = await getScreenshotImageInfo(label);
      const buffer = await getScreenshotImage(label);
      const bitmap = await createImageBitmap(
        new ImageData(new Uint8ClampedArray(buffer), info.width, info.height),
      );

      bitmapRef.current = bitmap;
      scaleRef.current =
        (window.innerWidth * (window.devicePixelRatio || 1)) / info.width;
      draw();
      await revealWhenPainted();
    } catch (error) {
      log.error("load screenshot pin failed", error);
    }
  });

  useUnmount(() => {
    bitmapRef.current?.close();
  });

  useEffect(() => {
    if (scalePercent === null) return;

    const timer = window.setTimeout(() => {
      setScalePercent(null);
    }, SCALE_BADGE_MS);

    return () => {
      window.clearTimeout(timer);
    };
  }, [scalePercent]);

  const revealWhenPainted = async () => {
    try {
      await notifyScreenshotWindowReady(label);
    } catch (error) {
      log.error("reveal screenshot pin failed", error);
    }
  };

  const close = async () => {
    try {
      await closeScreenshotWindow(label);
    } catch (error) {
      log.error("close screenshot pin failed", error);
    }
  };

  const handlePointerDown = (event: ReactPointerEvent<HTMLElement>) => {
    if (event.button !== 0) return;

    pressRef.current = { x: event.clientX, y: event.clientY };
  };

  // Moving starts only after the pointer travels, so double-click still reaches the page.
  const handlePointerMove = async (event: ReactPointerEvent<HTMLElement>) => {
    const press = pressRef.current;
    if (!press || (event.buttons & 1) === 0) {
      pressRef.current = null;
      return;
    }

    const distance = Math.hypot(
      event.clientX - press.x,
      event.clientY - press.y,
    );
    if (distance < DRAG_THRESHOLD_CSS) return;

    pressRef.current = null;

    try {
      await currentWindow.startDragging();
    } catch (error) {
      log.error("move screenshot pin failed", error);
    }
  };

  const handlePointerUp = () => {
    pressRef.current = null;
  };

  const handleDoubleClick = () => {
    void close();
  };

  const handleWheel = async (event: ReactWheelEvent<HTMLElement>) => {
    const factor = event.deltaY < 0 ? WHEEL_STEP : 1 / WHEEL_STEP;
    const nextScale = Math.min(
      MAX_SCALE,
      Math.max(MIN_SCALE, scaleRef.current * factor),
    );
    const anchorX = event.clientX / Math.max(1, window.innerWidth);
    const anchorY = event.clientY / Math.max(1, window.innerHeight);

    scaleRef.current = nextScale;
    setScalePercent(Math.round(nextScale * 100));

    try {
      await setScreenshotPinScale(label, nextScale, anchorX, anchorY);
    } catch (error) {
      log.error("scale screenshot pin failed", error);
    }
  };

  const handleContextMenu = async (event: ReactMouseEvent<HTMLElement>) => {
    event.preventDefault();

    try {
      await showScreenshotPinMenu(label);
    } catch (error) {
      log.error("open screenshot pin menu failed", error);
    }
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;

    void close();
  };

  useEventListener("resize", draw);
  useEventListener("keydown", handleKeyDown);

  return (
    <main
      className="relative h-screen cursor-move select-none overflow-hidden bg-black"
      onContextMenu={handleContextMenu}
      onDoubleClick={handleDoubleClick}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onWheel={handleWheel}
    >
      <canvas className="block h-full w-full" ref={canvasRef} />
      <div className="pointer-events-none absolute inset-0 border border-white/25" />
      <div
        className={cn(
          "pointer-events-none absolute top-2 right-2 rounded-full bg-black/70 px-2 py-0.5 text-white text-xs opacity-0 transition-opacity",
          { "opacity-100": scalePercent !== null },
        )}
      >
        {scalePercent ?? Math.round(scaleRef.current * 100)}%
      </div>
    </main>
  );
};

export default ScreenshotPin;
