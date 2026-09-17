import type {
  FC,
  MouseEvent as ReactMouseEvent,
  ReactNode,
  PointerEvent as ReactPointerEvent,
} from "react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { PixelPoint, PixelRect } from "@/utils/pixelRect";
import type {
  CanvasInteraction,
  PointerInfo,
} from "../hooks/useCanvasInteraction";
import type { PixelSpans } from "../model/pixels";
import type { Shape } from "../model/shapes";
import {
  panViewport,
  toImagePoint,
  type Viewport,
  zoomViewportAt,
} from "../model/viewport";
import type { EditorScene } from "../render/drawDocument";
import { drawView } from "../render/drawView";

export interface ViewSize {
  width: number;
  height: number;
  ratio: number;
}

interface EditorCanvasProps {
  checker: readonly [string, string];
  children?: ReactNode;
  interaction: CanvasInteraction;
  measure: PixelSpans | null;
  panHeld: boolean;
  scene: EditorScene;
  selectedShape: Shape | null;
  selection: PixelRect | null;
  viewport: Viewport | null;
  zoomHeld: boolean;
  onFirstPaint: () => void;
  onHoverPoint: (point: PixelPoint | null) => void;
  onViewportChange: (viewport: Viewport) => void;
  onViewSizeChange: (size: ViewSize) => void;
}

interface PanGesture {
  lastX: number;
  lastY: number;
}

const HANDLE_HIT_CSS = 8;
const WHEEL_ZOOM_SPEED = 0.0015;

/** Canvas that shows the document, handles zoom and pan, and forwards edits to the tools. */
const EditorCanvas: FC<EditorCanvasProps> = (props) => {
  const {
    checker,
    children,
    interaction,
    measure,
    panHeld,
    scene,
    selectedShape,
    selection,
    viewport,
    zoomHeld,
    onFirstPaint,
    onHoverPoint,
    onViewportChange,
    onViewSizeChange,
  } = props;
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const panRef = useRef<PanGesture | null>(null);
  const editingRef = useRef(false);
  const paintedRef = useRef(false);
  const viewportRef = useRef(viewport);
  const [viewSize, setViewSize] = useState<ViewSize | null>(null);
  const [cursor, setCursor] = useState("crosshair");

  viewportRef.current = viewport;

  useLayoutEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const applySize = (size: ViewSize) => {
      setViewSize(size);
      onViewSizeChange(size);
    };

    // The window starts hidden, where ResizeObserver callbacks can be deferred,
    // so the first size comes from a synchronous layout read.
    const bounds = container.getBoundingClientRect();
    const initialRatio = window.devicePixelRatio || 1;
    applySize({
      height: Math.round(bounds.height * initialRatio),
      ratio: initialRatio,
      width: Math.round(bounds.width * initialRatio),
    });

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;

      const ratio = window.devicePixelRatio || 1;
      const deviceSize = entry.devicePixelContentBoxSize?.[0];
      applySize({
        height: Math.round(
          deviceSize?.blockSize ?? entry.contentRect.height * ratio,
        ),
        ratio,
        width: Math.round(
          deviceSize?.inlineSize ?? entry.contentRect.width * ratio,
        ),
      });
    });

    observer.observe(container);

    return () => {
      observer.disconnect();
    };
  }, [onViewSizeChange]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const handleWheel = (event: WheelEvent) => {
      const current = viewportRef.current;
      if (!current) return;

      event.preventDefault();

      const point = toDevicePoint(canvas, event.clientX, event.clientY);
      const ratio = window.devicePixelRatio || 1;

      if (event.ctrlKey || event.metaKey) {
        const factor = Math.exp(-event.deltaY * WHEEL_ZOOM_SPEED);
        onViewportChange(
          zoomViewportAt(current, current.zoom * factor, point.x, point.y),
        );

        return;
      }

      const dx = event.shiftKey ? event.deltaY : event.deltaX;
      const dy = event.shiftKey ? 0 : event.deltaY;
      onViewportChange(panViewport(current, -dx * ratio, -dy * ratio));
    };

    canvas.addEventListener("wheel", handleWheel, { passive: false });

    return () => {
      canvas.removeEventListener("wheel", handleWheel);
    };
  }, [onViewportChange]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context || !viewSize || !viewport) return;
    // A collapsed or minimized view has no pixels, and canvas patterns reject empty targets.
    if (viewSize.width <= 0 || viewSize.height <= 0) return;

    if (canvas.width !== viewSize.width) canvas.width = viewSize.width;
    if (canvas.height !== viewSize.height) canvas.height = viewSize.height;

    drawView(context, {
      checker,
      measure,
      ratio: viewSize.ratio,
      scene,
      selectedShape,
      selection,
      viewport,
    });

    if (paintedRef.current) return;

    // Animation frames can pause while the window is hidden, so report right after drawing.
    paintedRef.current = true;
    onFirstPaint();
  });

  const pointerInfo = (
    event:
      | ReactPointerEvent<HTMLCanvasElement>
      | ReactMouseEvent<HTMLCanvasElement>,
    current: Viewport,
  ): PointerInfo => {
    return {
      altKey: event.altKey,
      shiftKey: event.shiftKey,
      tolerance:
        (HANDLE_HIT_CSS * (window.devicePixelRatio || 1)) / current.zoom,
    };
  };

  const imagePointOf = (
    event:
      | ReactPointerEvent<HTMLCanvasElement>
      | ReactMouseEvent<HTMLCanvasElement>,
    current: Viewport,
  ) => {
    const canvas = event.currentTarget;
    const device = toDevicePoint(canvas, event.clientX, event.clientY);

    return { device, point: toImagePoint(current, device.x, device.y) };
  };

  const handlePointerDown = (event: ReactPointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas || !viewport) return;

    const { device, point } = imagePointOf(event, viewport);

    if (event.button === 1 || (event.button === 0 && panHeld)) {
      event.preventDefault();
      panRef.current = { lastX: device.x, lastY: device.y };
      canvas.setPointerCapture(event.pointerId);
      setCursor("grabbing");

      return;
    }

    if (event.button !== 0) return;

    if (zoomHeld) {
      const factor = event.altKey ? 0.5 : 2;
      onViewportChange(
        zoomViewportAt(viewport, viewport.zoom * factor, device.x, device.y),
      );

      return;
    }

    // Keep focus on the text editor until its own blur commits it.
    if (document.activeElement instanceof HTMLTextAreaElement) {
      event.preventDefault();
      document.activeElement.blur();
      return;
    }

    // The text tool focuses a new textarea synchronously; without this the
    // browser's default mousedown focus handling blurs it again immediately.
    event.preventDefault();
    editingRef.current = true;
    canvas.setPointerCapture(event.pointerId);
    interaction.pointerDown(point, pointerInfo(event, viewport));
  };

  const handlePointerMove = (event: ReactPointerEvent<HTMLCanvasElement>) => {
    if (!viewport) return;

    const { device, point } = imagePointOf(event, viewport);
    const pan = panRef.current;

    if (pan) {
      onViewportChange(
        panViewport(viewport, device.x - pan.lastX, device.y - pan.lastY),
      );
      panRef.current = { lastX: device.x, lastY: device.y };
      return;
    }

    onHoverPoint(point);
    interaction.hover(point);

    const info = pointerInfo(event, viewport);

    if (editingRef.current) {
      interaction.pointerMove(point, info);
      return;
    }

    setCursor(interaction.cursor(point, info));
  };

  const handlePointerUp = (event: ReactPointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (canvas?.hasPointerCapture(event.pointerId)) {
      canvas.releasePointerCapture(event.pointerId);
    }

    if (panRef.current) {
      panRef.current = null;
      setCursor(panHeld ? "grab" : "crosshair");
      return;
    }

    if (!editingRef.current || !viewport) return;

    editingRef.current = false;
    const { point } = imagePointOf(event, viewport);
    const info = pointerInfo(event, viewport);
    interaction.pointerUp(point, info);
    setCursor(interaction.cursor(point, info));
  };

  const handleDoubleClick = (event: ReactMouseEvent<HTMLCanvasElement>) => {
    if (!viewport || panHeld || zoomHeld) return;

    const { point } = imagePointOf(event, viewport);
    interaction.doubleClick(point, pointerInfo(event, viewport));
  };

  const handlePointerLeave = () => {
    if (editingRef.current || panRef.current) return;

    onHoverPoint(null);
    interaction.hover(null);
  };

  return (
    <div className="relative min-h-0 flex-1 overflow-hidden" ref={containerRef}>
      <canvas
        className="absolute inset-0 block h-full w-full touch-none"
        onDoubleClick={handleDoubleClick}
        onPointerCancel={handlePointerUp}
        onPointerDown={handlePointerDown}
        onPointerLeave={handlePointerLeave}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        ref={canvasRef}
        style={{ cursor: panHeld ? "grab" : zoomHeld ? "zoom-in" : cursor }}
      />
      {children}
    </div>
  );
};

function toDevicePoint(
  canvas: HTMLCanvasElement,
  clientX: number,
  clientY: number,
) {
  const bounds = canvas.getBoundingClientRect();

  return {
    x: ((clientX - bounds.left) * canvas.width) / Math.max(1, bounds.width),
    y: ((clientY - bounds.top) * canvas.height) / Math.max(1, bounds.height),
  };
}

export default EditorCanvas;
