/** A point in image pixels. Edge coordinates sit on pixel boundaries. */
export interface PixelPoint {
  x: number;
  y: number;
}

/** A rectangle covering pixels `[x, x + width)` and `[y, y + height)`. */
export interface PixelRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type RectHandle = "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw";

export type ArrowKey = "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight";

const clamp = (value: number, min: number, max: number) => {
  return Math.min(Math.max(value, min), max);
};

export const isArrowKey = (key: string): key is ArrowKey => {
  return (
    key === "ArrowUp" ||
    key === "ArrowDown" ||
    key === "ArrowLeft" ||
    key === "ArrowRight"
  );
};

/** Snaps a continuous image point to the nearest pixel edge inside `bounds`. */
export const snapEdgeInside = (
  point: PixelPoint,
  bounds: PixelRect,
): PixelPoint => {
  return {
    x: clamp(Math.round(point.x), bounds.x, bounds.x + bounds.width),
    y: clamp(Math.round(point.y), bounds.y, bounds.y + bounds.height),
  };
};

/** Builds a rectangle from two edge points in either drag direction. */
export const rectFromEdges = (from: PixelPoint, to: PixelPoint): PixelRect => {
  return {
    height: Math.abs(to.y - from.y),
    width: Math.abs(to.x - from.x),
    x: Math.min(from.x, to.x),
    y: Math.min(from.y, to.y),
  };
};

export const isEmptyRect = (rect: PixelRect) => {
  return rect.width <= 0 || rect.height <= 0;
};

export const rectContainsPoint = (rect: PixelRect, point: PixelPoint) => {
  return (
    point.x >= rect.x &&
    point.y >= rect.y &&
    point.x < rect.x + rect.width &&
    point.y < rect.y + rect.height
  );
};

export const intersectRects = (
  a: PixelRect,
  b: PixelRect,
): PixelRect | null => {
  const left = Math.max(a.x, b.x);
  const top = Math.max(a.y, b.y);
  const right = Math.min(a.x + a.width, b.x + b.width);
  const bottom = Math.min(a.y + a.height, b.y + b.height);

  if (right <= left || bottom <= top) return null;

  return { height: bottom - top, width: right - left, x: left, y: top };
};

/** Moves a rectangle by whole pixels without leaving `bounds`. */
export const moveRectInside = (
  rect: PixelRect,
  dx: number,
  dy: number,
  bounds: PixelRect,
): PixelRect => {
  return {
    ...rect,
    x: clamp(rect.x + dx, bounds.x, bounds.x + bounds.width - rect.width),
    y: clamp(rect.y + dy, bounds.y, bounds.y + bounds.height - rect.height),
  };
};

/** Drags one edge or corner of `rect` to `edge`, flipping when it crosses the opposite side. */
export const resizeRectToEdge = (
  rect: PixelRect,
  handle: RectHandle,
  edge: PixelPoint,
  bounds: PixelRect,
): PixelRect => {
  const target = snapEdgeInside(edge, bounds);
  let left = rect.x;
  let top = rect.y;
  let right = rect.x + rect.width;
  let bottom = rect.y + rect.height;

  if (handle.includes("w")) left = target.x;
  if (handle.includes("e")) right = target.x;
  if (handle.includes("n")) top = target.y;
  if (handle.includes("s")) bottom = target.y;

  return rectFromEdges({ x: left, y: top }, { x: right, y: bottom });
};

/** Finds the handle under `point`; corners win over edges. `tolerance` is in image pixels. */
export const hitRectHandle = (
  rect: PixelRect,
  point: PixelPoint,
  tolerance: number,
): RectHandle | null => {
  const left = rect.x;
  const top = rect.y;
  const right = rect.x + rect.width;
  const bottom = rect.y + rect.height;
  const nearLeft = Math.abs(point.x - left) <= tolerance;
  const nearRight = Math.abs(point.x - right) <= tolerance;
  const nearTop = Math.abs(point.y - top) <= tolerance;
  const nearBottom = Math.abs(point.y - bottom) <= tolerance;
  const withinX = point.x >= left - tolerance && point.x <= right + tolerance;
  const withinY = point.y >= top - tolerance && point.y <= bottom + tolerance;

  if (nearTop && nearLeft) return "nw";
  if (nearTop && nearRight) return "ne";
  if (nearBottom && nearLeft) return "sw";
  if (nearBottom && nearRight) return "se";
  if (nearTop && withinX) return "n";
  if (nearBottom && withinX) return "s";
  if (nearLeft && withinY) return "w";
  if (nearRight && withinY) return "e";

  return null;
};

/** Arrow keys move a rectangle; with `resize` they grow or shrink its right or bottom edge. */
export const nudgeRect = (
  rect: PixelRect,
  key: ArrowKey,
  step: number,
  resize: boolean,
  bounds: PixelRect,
): PixelRect => {
  const horizontal = key === "ArrowLeft" || key === "ArrowRight";
  const direction = key === "ArrowLeft" || key === "ArrowUp" ? -1 : 1;

  if (!resize) {
    return horizontal
      ? moveRectInside(rect, direction * step, 0, bounds)
      : moveRectInside(rect, 0, direction * step, bounds);
  }

  if (horizontal) {
    const maxWidth = bounds.x + bounds.width - rect.x;

    return {
      ...rect,
      width: clamp(rect.width + direction * step, 1, maxWidth),
    };
  }

  const maxHeight = bounds.y + bounds.height - rect.y;

  return {
    ...rect,
    height: clamp(rect.height + direction * step, 1, maxHeight),
  };
};
