import type { PixelPoint, PixelRect } from "@/utils/pixelRect";
import { averageBorderColor, type PixelSpans } from "./pixels";
import {
  type BlurShape,
  boxFromPoints,
  constrainAngle,
  createShapeId,
  isDegenerateShape,
  type MagnifierShape,
  nextCounterValue,
  type RulerShape,
  type Shape,
  snapBox,
} from "./shapes";
import type { EditorTool, ToolStyle } from "./tools";

export interface DragInfo {
  shiftKey: boolean;
  /** Image pixels covered by the handle hit radius at the current zoom. */
  tolerance: number;
}

export interface SourceImage {
  height: number;
  pixels: Uint8ClampedArray;
  width: number;
}

/** A press that travels less than this many handle radii counts as a click. */
export const CLICK_DISTANCE = 0.5;

export const isSelectTool = (tool: EditorTool) => {
  return tool === "select" || tool === "backdrop";
};

/**
 * Whether a press on `hit` moves it instead of starting a new shape. Select
 * tools grab anything; drawing tools grab the selected shape or their own kind.
 * Freehand strokes often start on earlier strokes, so the pen never grabs.
 */
export const grabsShape = (
  hit: Shape,
  tool: EditorTool,
  selectedId: string | null,
) => {
  if (tool === "pen") return false;

  return isSelectTool(tool) || hit.id === selectedId || hit.kind === tool;
};

const rulerStyle = (style: ToolStyle, scale: number) => {
  return {
    color: style.color,
    fontSize: Math.max(11, style.strokeWidth * 3 + 8) * scale,
    strokeWidth: Math.max(1, style.strokeWidth / 2) * scale,
  };
};

/** The zero-size shape a drawing tool starts at the press point. */
export const createShape = (
  tool: EditorTool,
  point: PixelPoint,
  style: ToolStyle,
  scale: number,
  shapes: readonly Shape[],
): Shape | null => {
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
        ...rulerStyle(style, scale),
        from: point,
        id,
        kind: "ruler",
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
        value: nextCounterValue(shapes),
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

/** The shape being created after the pointer moved from `origin` to `point`. */
export const dragCreatedShape = (
  shape: Shape,
  origin: PixelPoint,
  point: PixelPoint,
  info: DragInfo,
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
      if (Math.hypot(point.x - last.x, point.y - last.y) < info.tolerance / 4) {
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

/** Finishes a created shape on release, or `null` when it is too small to keep. */
export const finalizeCreatedShape = (
  shape: Shape,
  info: DragInfo,
  image: SourceImage,
  area: PixelRect,
  scale: number,
): Shape | null => {
  switch (shape.kind) {
    case "blur": {
      const next = refreshBlur(shape, image);

      return isDegenerateShape(next, 2) ? null : next;
    }
    case "magnifier":
      return placeMagnifier(shape, info, area, scale);
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
export const placeMagnifier = (
  shape: MagnifierShape,
  info: DragInfo,
  area: PixelRect,
  scale: number,
): MagnifierShape => {
  const clicked = shape.width < info.tolerance && shape.height < info.tolerance;
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

/** Keeps derived values current after a shape changes, such as erase colors. */
export const refreshShape = (shape: Shape, image: SourceImage): Shape => {
  return shape.kind === "blur" ? refreshBlur(shape, image) : shape;
};

/** Snaps a blur to whole pixels and samples the erase color around it. */
const refreshBlur = (shape: BlurShape, image: SourceImage): BlurShape => {
  const box = snapBox(shape);

  return {
    ...shape,
    ...box,
    fillColor:
      shape.mode === "erase"
        ? averageBorderColor(image.pixels, image.width, image.height, box)
        : shape.fillColor,
  };
};

/** A horizontal and a vertical ruler across the measured surface. */
export const measureRulers = (
  spans: PixelSpans,
  style: ToolStyle,
  scale: number,
): RulerShape[] => {
  const centerX = spans.left + (spans.right - spans.left) / 2;
  const centerY = spans.top + (spans.bottom - spans.top) / 2;
  const base = { ...rulerStyle(style, scale), kind: "ruler" as const };

  return [
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
  ];
};
