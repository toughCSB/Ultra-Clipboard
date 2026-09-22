import { useRef } from "react";
import {
  hitRectHandle,
  moveRectInside,
  type PixelPoint,
  type PixelRect,
  type RectHandle,
  rectContainsPoint,
  rectFromEdges,
  resizeRectToEdge,
  snapEdgeInside,
} from "@/utils/pixelRect";
import {
  addShape,
  addShapes,
  documentBounds,
  type EditorDocument,
  findShape,
  replaceShape,
} from "../model/document";
import {
  averageBorderColor,
  measureSpans,
  type PixelSpans,
} from "../model/pixels";
import {
  type BlurShape,
  boxFromPoints,
  constrainAngle,
  createShapeId,
  hitShapeHandle,
  isDegenerateShape,
  type MagnifierShape,
  moveShape,
  nextCounterValue,
  resizeShape,
  type Shape,
  type ShapeHandle,
  snapBox,
  topShapeAt,
} from "../model/shapes";
import type { EditorTool, ToolStyle } from "../model/tools";

export interface PointerInfo {
  altKey: boolean;
  shiftKey: boolean;
  /** Image pixels covered by the handle hit radius at the current zoom. */
  tolerance: number;
}

export interface TextEditRequest {
  point: PixelPoint;
  shape: Extract<Shape, { kind: "text" }> | null;
}

export interface CanvasInteraction {
  cursor: (point: PixelPoint, info: PointerInfo) => string;
  doubleClick: (point: PixelPoint, info: PointerInfo) => void;
  hover: (point: PixelPoint | null) => void;
  pointerDown: (point: PixelPoint, info: PointerInfo) => void;
  pointerMove: (point: PixelPoint, info: PointerInfo) => void;
  pointerUp: (point: PixelPoint, info: PointerInfo) => void;
}

interface InteractionOptions {
  document: EditorDocument;
  image: { height: number; pixels: Uint8ClampedArray; width: number };
  /** Device pixels per logical pixel of the capture, used to size new annotations. */
  scale: number;
  selectedId: string | null;
  selection: PixelRect | null;
  style: ToolStyle;
  tool: EditorTool;
  commit: (document: EditorDocument) => void;
  onEditText: (request: TextEditRequest) => void;
  onMeasure: (spans: PixelSpans | null) => void;
  onSelectShape: (id: string | null) => void;
  onSelectionChange: (selection: PixelRect | null) => void;
  setDraft: (document: EditorDocument | null) => void;
}

type Gesture =
  | { kind: "area"; origin: PixelPoint }
  | { kind: "moveArea"; origin: PixelPoint; rect: PixelRect }
  | { handle: RectHandle; kind: "resizeArea"; rect: PixelRect }
  | { kind: "moveShape"; moved: boolean; origin: PixelPoint; shape: Shape }
  | { handle: ShapeHandle; kind: "resizeShape"; shape: Shape }
  | { kind: "create"; origin: PixelPoint; shape: Shape };

const HANDLE_CURSORS: Record<RectHandle, string> = {
  e: "ew-resize",
  n: "ns-resize",
  ne: "nesw-resize",
  nw: "nwse-resize",
  s: "ns-resize",
  se: "nwse-resize",
  sw: "nesw-resize",
  w: "ew-resize",
};

/** Colors that differ by at most this much in every channel count as one surface. */
const MEASURE_TOLERANCE = 12;
/** A press that travels less than this many handle radii counts as a click. */
const CLICK_DISTANCE = 0.5;

/**
 * Turns pointer input in image coordinates into selections and annotations.
 * Drags show a draft document; releasing the pointer commits one undo step.
 */
export const useCanvasInteraction = (
  options: InteractionOptions,
): CanvasInteraction => {
  const gestureRef = useRef<Gesture | null>(null);
  const draftRef = useRef<EditorDocument | null>(null);
  const optionsRef = useRef(options);

  optionsRef.current = options;

  const setDraft = (document: EditorDocument | null) => {
    draftRef.current = document;
    optionsRef.current.setDraft(document);
  };

  const finishDraft = () => {
    const draft = draftRef.current;
    setDraft(null);
    if (draft) optionsRef.current.commit(draft);
  };

  const bounds = () => documentBounds(optionsRef.current.document);

  const createShape = (point: PixelPoint): Shape | null => {
    const { document, scale, style, tool } = optionsRef.current;
    const id = createShapeId();
    const strokeWidth = style.strokeWidth * scale;
    const box = { height: 0, width: 0, x: point.x, y: point.y };

    switch (tool) {
      case "arrow":
      case "line":
        return {
          color: style.color,
          from: point,
          id,
          kind: tool,
          strokeWidth,
          to: point,
        };
      case "ruler":
        return {
          color: style.color,
          fontSize: Math.max(11, style.strokeWidth * 3 + 8) * scale,
          from: point,
          id,
          kind: "ruler",
          strokeWidth: Math.max(1, style.strokeWidth / 2) * scale,
          to: point,
        };
      case "rect":
      case "oval":
        return {
          ...box,
          color: style.color,
          fill: style.rectFill,
          id,
          kind: tool,
          strokeWidth,
        };
      case "highlighter":
        return {
          ...box,
          color: style.highlightColor,
          id,
          kind: "highlighter",
          opacity: style.highlightOpacity,
        };
      case "blur":
        return {
          ...box,
          fillColor: "#FFFFFF",
          id,
          kind: "blur",
          mode: style.blurMode,
          strength: Math.max(4, style.strokeWidth * 3) * scale,
        };
      case "spotlight":
        return { ...box, id, kind: "spotlight", oval: style.spotlightOval };
      case "pen":
        return {
          color: style.color,
          id,
          kind: "pen",
          points: [point],
          strokeWidth,
        };
      case "counter":
        return {
          color: style.color,
          id,
          kind: "counter",
          radius: style.counterSize * scale,
          value: nextCounterValue(document.shapes),
          x: point.x,
          y: point.y,
        };
      case "magnifier":
        return {
          ...box,
          id,
          kind: "magnifier",
          sourceX: point.x,
          sourceY: point.y,
          strokeWidth: 3 * scale,
          zoom: style.magnifierZoom,
        };
      default:
        return null;
    }
  };

  const updateCreated = (
    shape: Shape,
    origin: PixelPoint,
    point: PixelPoint,
    info: PointerInfo,
  ): Shape => {
    switch (shape.kind) {
      case "arrow":
      case "line":
      case "ruler":
        return {
          ...shape,
          to: info.shiftKey ? constrainAngle(origin, point) : point,
        };
      case "pen": {
        const last = shape.points[shape.points.length - 1];
        if (
          Math.hypot(point.x - last.x, point.y - last.y) <
          info.tolerance / 4
        ) {
          return shape;
        }

        return { ...shape, points: [...shape.points, point] };
      }
      case "counter":
        return { ...shape, x: point.x, y: point.y };
      case "text":
      case "image":
        return shape;
      default:
        return { ...shape, ...boxFromPoints(origin, point, info.shiftKey) };
    }
  };

  /** Finishes a created shape once the pointer is released. */
  const finalizeCreated = (shape: Shape, info: PointerInfo): Shape | null => {
    const { image, scale } = optionsRef.current;

    switch (shape.kind) {
      case "blur": {
        const box = snapBox(shape);
        const next: BlurShape = { ...shape, ...box };
        if (next.mode === "erase") {
          next.fillColor = averageBorderColor(
            image.pixels,
            image.width,
            image.height,
            box,
          );
        }

        return isDegenerateShape(next, 2) ? null : next;
      }
      case "magnifier":
        return placeMagnifier(shape, info, scale);
      case "counter":
      case "pen":
        return shape;
      default:
        return isDegenerateShape(shape, info.tolerance * CLICK_DISTANCE)
          ? null
          : shape;
    }
  };

  /** Turns the dragged source area into a lens placed beside it. */
  const placeMagnifier = (
    shape: MagnifierShape,
    info: PointerInfo,
    scale: number,
  ): MagnifierShape => {
    const area = bounds();
    const clicked =
      shape.width < info.tolerance && shape.height < info.tolerance;
    const side = clicked ? 48 * scale : Math.max(shape.width, shape.height);
    const sourceX = clicked ? shape.x : shape.x + shape.width / 2;
    const sourceY = clicked ? shape.y : shape.y + shape.height / 2;
    const lens = Math.max(side * shape.zoom, 64 * scale);
    const gap = 16 * scale;
    const rightX = sourceX + side / 2 + gap;
    const x =
      rightX + lens <= area.x + area.width
        ? rightX
        : Math.max(area.x, sourceX - side / 2 - gap - lens);
    const y = Math.min(
      Math.max(area.y, sourceY - lens / 2),
      area.y + area.height - lens,
    );

    return { ...shape, height: lens, sourceX, sourceY, width: lens, x, y };
  };

  const measureAt = (point: PixelPoint) => {
    const { document, image } = optionsRef.current;

    return measureSpans(
      image.pixels,
      image.width,
      image.height,
      point,
      document.crop,
      MEASURE_TOLERANCE,
    );
  };

  const commitMeasure = (point: PixelPoint) => {
    const { document, scale, style } = optionsRef.current;
    const spans = measureAt(point);
    if (!spans) return;

    const centerX = spans.left + (spans.right - spans.left) / 2;
    const centerY = spans.top + (spans.bottom - spans.top) / 2;
    const base = {
      color: style.color,
      fontSize: Math.max(11, style.strokeWidth * 3 + 8) * scale,
      kind: "ruler" as const,
      strokeWidth: Math.max(1, style.strokeWidth / 2) * scale,
    };

    optionsRef.current.commit(
      addShapes(document, [
        {
          ...base,
          from: { x: spans.left, y: centerY },
          id: createShapeId(),
          to: { x: spans.right, y: centerY },
        },
        {
          ...base,
          from: { x: centerX, y: spans.top },
          id: createShapeId(),
          to: { x: centerX, y: spans.bottom },
        },
      ]),
    );
  };

  /** Starts moving or resizing the selected shape, or selects the shape under the press. */
  const beginShapeGesture = (
    point: PixelPoint,
    info: PointerInfo,
    selectAny: boolean,
  ) => {
    const { document, selectedId } = optionsRef.current;
    const selected = findShape(document, selectedId);

    if (selected) {
      const handle = hitShapeHandle(selected, point, info.tolerance);
      if (handle) {
        gestureRef.current = { handle, kind: "resizeShape", shape: selected };
        return true;
      }
    }

    const hit = topShapeAt(document.shapes, point, info.tolerance);
    const target =
      hit &&
      (selectAny ||
        hit.id === selectedId ||
        hit.kind === optionsRef.current.tool)
        ? hit
        : null;
    if (!target) return false;

    optionsRef.current.onSelectShape(target.id);
    gestureRef.current = {
      kind: "moveShape",
      moved: false,
      origin: point,
      shape: target,
    };

    return true;
  };

  const pointerDown = (point: PixelPoint, info: PointerInfo) => {
    const { document, selection, tool } = optionsRef.current;

    if (tool === "select" || tool === "backdrop") {
      if (beginShapeGesture(point, info, true)) return;

      optionsRef.current.onSelectShape(null);

      if (selection) {
        const handle = hitRectHandle(selection, point, info.tolerance);
        if (handle) {
          gestureRef.current = { handle, kind: "resizeArea", rect: selection };
          return;
        }
        if (rectContainsPoint(selection, point)) {
          gestureRef.current = {
            kind: "moveArea",
            origin: point,
            rect: selection,
          };
          return;
        }
      }

      gestureRef.current = {
        kind: "area",
        origin: snapEdgeInside(point, bounds()),
      };
      optionsRef.current.onSelectionChange(null);
      return;
    }

    optionsRef.current.onSelectionChange(null);

    if (tool === "text") {
      const hit = topShapeAt(document.shapes, point, info.tolerance);
      if (hit?.kind === "text") {
        optionsRef.current.onSelectShape(hit.id);
        gestureRef.current = {
          kind: "moveShape",
          moved: false,
          origin: point,
          shape: hit,
        };
        return;
      }

      optionsRef.current.onSelectShape(null);
      optionsRef.current.onEditText({ point, shape: null });
      return;
    }

    // Freehand strokes often start on earlier strokes, so the pen never grabs shapes.
    if (tool !== "pen" && beginShapeGesture(point, info, false)) return;

    const shape = createShape(point);
    if (!shape) return;

    optionsRef.current.onSelectShape(null);
    optionsRef.current.onMeasure(null);
    gestureRef.current = { kind: "create", origin: point, shape };
    if (shape.kind === "pen") setDraft(addShape(document, shape));
  };

  const pointerMove = (point: PixelPoint, info: PointerInfo) => {
    const gesture = gestureRef.current;
    const { document } = optionsRef.current;

    if (!gesture) return;

    switch (gesture.kind) {
      case "area": {
        const rect = rectFromEdges(
          gesture.origin,
          snapEdgeInside(point, bounds()),
        );
        optionsRef.current.onSelectionChange(
          rect.width > 0 && rect.height > 0 ? rect : null,
        );
        return;
      }
      case "moveArea": {
        const dx = Math.round(point.x - gesture.origin.x);
        const dy = Math.round(point.y - gesture.origin.y);
        optionsRef.current.onSelectionChange(
          moveRectInside(gesture.rect, dx, dy, bounds()),
        );
        return;
      }
      case "resizeArea": {
        const rect = resizeRectToEdge(
          gesture.rect,
          gesture.handle,
          point,
          bounds(),
        );
        optionsRef.current.onSelectionChange(
          rect.width > 0 && rect.height > 0 ? rect : null,
        );
        return;
      }
      case "moveShape": {
        let dx = point.x - gesture.origin.x;
        let dy = point.y - gesture.origin.y;
        if (
          !gesture.moved &&
          Math.hypot(dx, dy) < info.tolerance * CLICK_DISTANCE
        ) {
          return;
        }
        if (info.shiftKey) {
          if (Math.abs(dx) > Math.abs(dy)) dy = 0;
          else dx = 0;
        }

        gestureRef.current = { ...gesture, moved: true };
        setDraft(
          replaceShape(
            document,
            refreshShape(moveShape(gesture.shape, dx, dy)),
          ),
        );
        return;
      }
      case "resizeShape":
        setDraft(
          replaceShape(
            document,
            refreshShape(
              resizeShape(gesture.shape, gesture.handle, point, info.shiftKey),
            ),
          ),
        );
        return;
      case "create": {
        const shape = updateCreated(gesture.shape, gesture.origin, point, info);
        gestureRef.current = { ...gesture, shape };
        setDraft(addShape(document, shape));
        return;
      }
    }
  };

  /** Keeps derived values current after a shape moves, such as erase colors. */
  const refreshShape = (shape: Shape): Shape => {
    if (shape.kind !== "blur") return shape;

    const box = snapBox(shape);
    const { image } = optionsRef.current;

    return {
      ...shape,
      ...box,
      fillColor:
        shape.mode === "erase"
          ? averageBorderColor(image.pixels, image.width, image.height, box)
          : shape.fillColor,
    };
  };

  const pointerUp = (point: PixelPoint, info: PointerInfo) => {
    const gesture = gestureRef.current;
    gestureRef.current = null;
    if (!gesture) return;

    const { document } = optionsRef.current;

    switch (gesture.kind) {
      case "moveShape":
      case "resizeShape":
        finishDraft();
        return;
      case "create": {
        const shape = updateCreated(gesture.shape, gesture.origin, point, info);
        const moved =
          Math.hypot(point.x - gesture.origin.x, point.y - gesture.origin.y) >=
          info.tolerance * CLICK_DISTANCE;

        setDraft(null);

        if (shape.kind === "ruler" && !moved) {
          commitMeasure(point);
          return;
        }

        const finished = finalizeCreated(shape, info);
        if (!finished) return;

        optionsRef.current.commit(addShape(document, finished));
        if (finished.kind !== "pen") {
          optionsRef.current.onSelectShape(finished.id);
        }
        return;
      }
      default:
        return;
    }
  };

  const doubleClick = (point: PixelPoint, info: PointerInfo) => {
    const { document } = optionsRef.current;
    const hit = topShapeAt(document.shapes, point, info.tolerance);
    if (hit?.kind !== "text") return;

    gestureRef.current = null;
    setDraft(null);
    optionsRef.current.onSelectShape(hit.id);
    optionsRef.current.onEditText({ point, shape: hit });
  };

  const hover = (point: PixelPoint | null) => {
    const { tool } = optionsRef.current;
    if (tool !== "ruler") return;

    optionsRef.current.onMeasure(
      point && !gestureRef.current ? measureAt(point) : null,
    );
  };

  const cursor = (point: PixelPoint, info: PointerInfo) => {
    const { document, selectedId, selection, tool } = optionsRef.current;
    const selected = findShape(document, selectedId);

    if (selected) {
      const handle = hitShapeHandle(selected, point, info.tolerance);
      if (handle === "start" || handle === "end") return "grab";
      if (handle) return HANDLE_CURSORS[handle];
    }

    const hit = topShapeAt(document.shapes, point, info.tolerance);
    const selectTool = tool === "select" || tool === "backdrop";

    if (
      hit &&
      tool !== "pen" &&
      (selectTool || hit.id === selectedId || hit.kind === tool)
    ) {
      return "move";
    }

    if (tool === "text") return "text";
    if (!selectTool) return "crosshair";
    if (!selection) return "crosshair";

    const handle = hitRectHandle(selection, point, info.tolerance);
    if (handle) return HANDLE_CURSORS[handle];

    return rectContainsPoint(selection, point) ? "move" : "crosshair";
  };

  return { cursor, doubleClick, hover, pointerDown, pointerMove, pointerUp };
};
