import type { PixelPoint, RectHandle } from "../../../utils/pixelRect";

/** Axis-aligned box in image pixels. Unlike `PixelRect`, values may be fractional. */
export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type BlurMode = "blur" | "pixelate" | "erase";

export type TextStyle = "plain" | "outline" | "background";

interface ShapeBase {
  id: string;
}

export interface RectShape extends ShapeBase, Box {
  kind: "rect" | "oval";
  color: string;
  strokeWidth: number;
  fill: boolean;
}

export interface HighlighterShape extends ShapeBase, Box {
  kind: "highlighter";
  color: string;
}

export interface BlurShape extends ShapeBase, Box {
  kind: "blur";
  mode: BlurMode;
  /** Blur radius or pixel block size, in image pixels. */
  strength: number;
  /** Solid color used by the erase mode, sampled around the box. */
  fillColor: string;
}

export interface SpotlightShape extends ShapeBase, Box {
  kind: "spotlight";
  oval: boolean;
}

export interface ImageShape extends ShapeBase, Box {
  kind: "image";
  bitmap: ImageBitmap;
}

export interface MagnifierShape extends ShapeBase, Box {
  kind: "magnifier";
  /** Center of the magnified area in image pixels. */
  sourceX: number;
  sourceY: number;
  zoom: number;
  strokeWidth: number;
}

export interface TextShape extends ShapeBase, Box {
  kind: "text";
  text: string;
  color: string;
  fontSize: number;
  style: TextStyle;
}

export interface LineShape extends ShapeBase {
  kind: "arrow" | "line";
  from: PixelPoint;
  to: PixelPoint;
  color: string;
  strokeWidth: number;
}

export interface RulerShape extends ShapeBase {
  kind: "ruler";
  from: PixelPoint;
  to: PixelPoint;
  color: string;
  strokeWidth: number;
  fontSize: number;
}

export interface PenShape extends ShapeBase {
  kind: "pen";
  points: readonly PixelPoint[];
  color: string;
  strokeWidth: number;
}

export interface CounterShape extends ShapeBase {
  kind: "counter";
  x: number;
  y: number;
  radius: number;
  value: number;
  color: string;
}

export type Shape =
  | BlurShape
  | CounterShape
  | HighlighterShape
  | ImageShape
  | LineShape
  | MagnifierShape
  | PenShape
  | RectShape
  | RulerShape
  | SpotlightShape
  | TextShape;

export type ShapeKind = Shape["kind"];

export type ShapeHandle = RectHandle | "start" | "end";

const BOX_HANDLES: RectHandle[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];
const CORNER_HANDLES: RectHandle[] = ["nw", "ne", "se", "sw"];
const MIN_SIDE = 1;

let idSequence = 0;

export const createShapeId = () => {
  idSequence += 1;

  return `shape-${Date.now().toString(36)}-${idSequence}`;
};

export const isBoxShape = (
  shape: Shape,
): shape is
  | BlurShape
  | HighlighterShape
  | ImageShape
  | MagnifierShape
  | RectShape
  | SpotlightShape
  | TextShape => {
  return "width" in shape && "height" in shape;
};

export const isSegmentShape = (
  shape: Shape,
): shape is LineShape | RulerShape => {
  return (
    shape.kind === "arrow" || shape.kind === "line" || shape.kind === "ruler"
  );
};

/** Builds a box from two drag points; `square` keeps both sides equal. */
export const boxFromPoints = (
  from: PixelPoint,
  to: PixelPoint,
  square = false,
): Box => {
  let dx = to.x - from.x;
  let dy = to.y - from.y;

  if (square) {
    const side = Math.max(Math.abs(dx), Math.abs(dy));
    dx = Math.sign(dx || 1) * side;
    dy = Math.sign(dy || 1) * side;
  }

  return {
    height: Math.abs(dy),
    width: Math.abs(dx),
    x: Math.min(from.x, from.x + dx),
    y: Math.min(from.y, from.y + dy),
  };
};

/** Rounds a box outward to whole pixels. */
export const snapBox = (box: Box): Box => {
  const left = Math.floor(box.x);
  const top = Math.floor(box.y);
  const right = Math.ceil(box.x + box.width);
  const bottom = Math.ceil(box.y + box.height);

  return { height: bottom - top, width: right - left, x: left, y: top };
};

/** Snaps the segment direction to 45° steps while keeping its length. */
export const constrainAngle = (
  from: PixelPoint,
  to: PixelPoint,
): PixelPoint => {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const length = Math.hypot(dx, dy);
  if (length === 0) return to;

  const step = Math.PI / 4;
  const angle = Math.round(Math.atan2(dy, dx) / step) * step;

  return {
    x: from.x + Math.round(Math.cos(angle) * length * 1e6) / 1e6,
    y: from.y + Math.round(Math.sin(angle) * length * 1e6) / 1e6,
  };
};

export const shapeBounds = (shape: Shape): Box => {
  switch (shape.kind) {
    case "arrow":
    case "line":
    case "ruler": {
      const pad = shape.strokeWidth / 2;
      const box = boxFromPoints(shape.from, shape.to);

      return inflateBox(box, pad);
    }
    case "pen": {
      const xs = shape.points.map((point) => point.x);
      const ys = shape.points.map((point) => point.y);
      const left = Math.min(...xs);
      const top = Math.min(...ys);

      return inflateBox(
        {
          height: Math.max(...ys) - top,
          width: Math.max(...xs) - left,
          x: left,
          y: top,
        },
        shape.strokeWidth / 2,
      );
    }
    case "counter":
      return {
        height: shape.radius * 2,
        width: shape.radius * 2,
        x: shape.x - shape.radius,
        y: shape.y - shape.radius,
      };
    default:
      return {
        height: shape.height,
        width: shape.width,
        x: shape.x,
        y: shape.y,
      };
  }
};

export const inflateBox = (box: Box, amount: number): Box => {
  return {
    height: box.height + amount * 2,
    width: box.width + amount * 2,
    x: box.x - amount,
    y: box.y - amount,
  };
};

export const boxContains = (box: Box, point: PixelPoint) => {
  return (
    point.x >= box.x &&
    point.y >= box.y &&
    point.x <= box.x + box.width &&
    point.y <= box.y + box.height
  );
};

export const distanceToSegment = (
  point: PixelPoint,
  from: PixelPoint,
  to: PixelPoint,
) => {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const lengthSquared = dx * dx + dy * dy;
  const t =
    lengthSquared === 0
      ? 0
      : Math.max(
          0,
          Math.min(
            1,
            ((point.x - from.x) * dx + (point.y - from.y) * dy) / lengthSquared,
          ),
        );

  return Math.hypot(point.x - (from.x + t * dx), point.y - (from.y + t * dy));
};

/** Whether `point` touches the visible part of `shape`. `tolerance` is in image pixels. */
export const hitShape = (
  shape: Shape,
  point: PixelPoint,
  tolerance: number,
) => {
  switch (shape.kind) {
    case "arrow":
    case "line":
    case "ruler":
      return (
        distanceToSegment(point, shape.from, shape.to) <=
        shape.strokeWidth / 2 + tolerance
      );
    case "pen": {
      const reach = shape.strokeWidth / 2 + tolerance;
      if (shape.points.length === 1) {
        return (
          Math.hypot(
            point.x - shape.points[0].x,
            point.y - shape.points[0].y,
          ) <= reach
        );
      }

      for (let index = 1; index < shape.points.length; index += 1) {
        if (
          distanceToSegment(
            point,
            shape.points[index - 1],
            shape.points[index],
          ) <= reach
        ) {
          return true;
        }
      }

      return false;
    }
    case "counter":
      return (
        Math.hypot(point.x - shape.x, point.y - shape.y) <=
        shape.radius + tolerance
      );
    case "rect": {
      const outer = inflateBox(shape, shape.strokeWidth / 2 + tolerance);
      if (!boxContains(outer, point)) return false;
      if (shape.fill) return true;

      const inner = inflateBox(shape, -(shape.strokeWidth / 2 + tolerance));

      return (
        inner.width <= 0 || inner.height <= 0 || !boxContains(inner, point)
      );
    }
    case "oval": {
      const rx = shape.width / 2;
      const ry = shape.height / 2;
      if (rx <= 0 || ry <= 0) return false;

      const distance = Math.hypot(
        (point.x - (shape.x + rx)) / rx,
        (point.y - (shape.y + ry)) / ry,
      );
      const reach = (shape.strokeWidth / 2 + tolerance) / Math.min(rx, ry);

      return shape.fill
        ? distance <= 1 + reach
        : Math.abs(distance - 1) <= reach;
    }
    case "magnifier": {
      const radius = Math.min(shape.width, shape.height) / 2;

      return (
        Math.hypot(
          point.x - (shape.x + shape.width / 2),
          point.y - (shape.y + shape.height / 2),
        ) <=
        radius + tolerance
      );
    }
    default:
      return boxContains(inflateBox(shape, tolerance), point);
  }
};

/** Finds the top-most shape under `point`. */
export const topShapeAt = (
  shapes: readonly Shape[],
  point: PixelPoint,
  tolerance: number,
) => {
  for (let index = shapes.length - 1; index >= 0; index -= 1) {
    if (hitShape(shapes[index], point, tolerance)) return shapes[index];
  }

  return null;
};

/** Handles a selected shape exposes, in drawing order. */
export const shapeHandles = (
  shape: Shape,
): { handle: ShapeHandle; point: PixelPoint }[] => {
  if (isSegmentShape(shape)) {
    return [
      { handle: "start", point: shape.from },
      { handle: "end", point: shape.to },
    ];
  }

  if (
    shape.kind === "pen" ||
    shape.kind === "counter" ||
    shape.kind === "text"
  ) {
    return [];
  }

  const handles = shape.kind === "magnifier" ? CORNER_HANDLES : BOX_HANDLES;

  return handles.map((handle) => {
    return { handle, point: boxHandlePoint(shape, handle) };
  });
};

const boxHandlePoint = (box: Box, handle: RectHandle): PixelPoint => {
  const x = handle.includes("w")
    ? box.x
    : handle.includes("e")
      ? box.x + box.width
      : box.x + box.width / 2;
  const y = handle.includes("n")
    ? box.y
    : handle.includes("s")
      ? box.y + box.height
      : box.y + box.height / 2;

  return { x, y };
};

export const hitShapeHandle = (
  shape: Shape,
  point: PixelPoint,
  tolerance: number,
): ShapeHandle | null => {
  for (const { handle, point: handlePoint } of shapeHandles(shape)) {
    if (
      Math.abs(point.x - handlePoint.x) <= tolerance &&
      Math.abs(point.y - handlePoint.y) <= tolerance
    ) {
      return handle;
    }
  }

  return null;
};

export const moveShape = (shape: Shape, dx: number, dy: number): Shape => {
  switch (shape.kind) {
    case "arrow":
    case "line":
    case "ruler":
      return {
        ...shape,
        from: { x: shape.from.x + dx, y: shape.from.y + dy },
        to: { x: shape.to.x + dx, y: shape.to.y + dy },
      };
    case "pen":
      return {
        ...shape,
        points: shape.points.map((point) => {
          return { x: point.x + dx, y: point.y + dy };
        }),
      };
    default:
      return { ...shape, x: shape.x + dx, y: shape.y + dy };
  }
};

/**
 * Drags one handle of `original` to `point`. Box shapes flip when a handle crosses
 * the opposite side; `constrain` keeps the aspect ratio or snaps segment angles.
 */
export const resizeShape = (
  original: Shape,
  handle: ShapeHandle,
  point: PixelPoint,
  constrain: boolean,
): Shape => {
  if (isSegmentShape(original)) {
    if (handle === "start") {
      return {
        ...original,
        from: constrain ? constrainAngle(original.to, point) : point,
      };
    }

    return {
      ...original,
      to: constrain ? constrainAngle(original.from, point) : point,
    };
  }

  if (!isBoxShape(original) || handle === "start" || handle === "end") {
    return original;
  }

  const keepRatio =
    original.kind === "magnifier" || (constrain && original.kind !== "text");
  const box = resizeBox(original, handle, point, keepRatio);

  return { ...original, ...box };
};

const resizeBox = (
  box: Box,
  handle: RectHandle,
  point: PixelPoint,
  keepRatio: boolean,
): Box => {
  let left = box.x;
  let top = box.y;
  let right = box.x + box.width;
  let bottom = box.y + box.height;

  if (handle.includes("w")) left = point.x;
  if (handle.includes("e")) right = point.x;
  if (handle.includes("n")) top = point.y;
  if (handle.includes("s")) bottom = point.y;

  if (keepRatio && box.width > 0 && box.height > 0) {
    const ratio = box.width / box.height;
    const anchorX = handle.includes("w") ? right : left;
    const anchorY = handle.includes("n") ? bottom : top;
    const edgeOnly = handle.length === 1;
    const horizontal = handle === "e" || handle === "w";
    let width = Math.abs(right - left);
    let height = Math.abs(bottom - top);

    if (edgeOnly) {
      if (horizontal) height = width / ratio;
      else width = height * ratio;
    } else if (width / height > ratio) {
      height = width / ratio;
    } else {
      width = height * ratio;
    }

    const directionX = handle.includes("w")
      ? Math.sign(left - right || -1)
      : Math.sign(right - left || 1);
    const directionY = handle.includes("n")
      ? Math.sign(top - bottom || -1)
      : Math.sign(bottom - top || 1);

    if (edgeOnly && horizontal) {
      const centerY = box.y + box.height / 2;
      top = centerY - height / 2;
      bottom = centerY + height / 2;
      left = anchorX;
      right = anchorX + directionX * width;
    } else if (edgeOnly) {
      const centerX = box.x + box.width / 2;
      left = centerX - width / 2;
      right = centerX + width / 2;
      top = anchorY;
      bottom = anchorY + directionY * height;
    } else {
      left = anchorX;
      right = anchorX + directionX * width;
      top = anchorY;
      bottom = anchorY + directionY * height;
    }
  }

  const x = Math.min(left, right);
  const y = Math.min(top, bottom);

  return {
    height: Math.max(MIN_SIDE, Math.abs(bottom - top)),
    width: Math.max(MIN_SIDE, Math.abs(right - left)),
    x,
    y,
  };
};

/** The next counter number: one past the largest counter already placed. */
export const nextCounterValue = (shapes: readonly Shape[]) => {
  return (
    shapes.reduce((largest, shape) => {
      return shape.kind === "counter"
        ? Math.max(largest, shape.value)
        : largest;
    }, 0) + 1
  );
};

/** Whether the shape is too small to keep after a drag. */
export const isDegenerateShape = (shape: Shape, minimum: number) => {
  switch (shape.kind) {
    case "arrow":
    case "line":
    case "ruler":
      return (
        Math.hypot(shape.to.x - shape.from.x, shape.to.y - shape.from.y) <
        minimum
      );
    case "pen":
      return shape.points.length === 0;
    case "counter":
    case "text":
      return false;
    default:
      return shape.width < minimum || shape.height < minimum;
  }
};
