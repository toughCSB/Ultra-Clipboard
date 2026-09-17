import type { PixelPoint, PixelRect } from "../../../utils/pixelRect";

const toHex = (value: number) => {
  return value.toString(16).padStart(2, "0").toUpperCase();
};

/** Reads the `#RRGGBB` color of the pixel containing `point`, or `null` outside the image. */
export const readPixelHex = (
  pixels: Uint8ClampedArray,
  width: number,
  height: number,
  point: PixelPoint,
) => {
  const x = Math.floor(point.x);
  const y = Math.floor(point.y);

  if (x < 0 || y < 0 || x >= width || y >= height) return null;

  const index = (y * width + x) * 4;

  return `#${toHex(pixels[index])}${toHex(pixels[index + 1])}${toHex(pixels[index + 2])}`;
};

/**
 * Averages the pixels on the one-pixel ring just outside `rect`, clipped to the image.
 * Erasing with this color blends the box into a flat background.
 */
export const averageBorderColor = (
  pixels: Uint8ClampedArray,
  width: number,
  height: number,
  rect: PixelRect,
) => {
  const left = Math.max(0, rect.x - 1);
  const top = Math.max(0, rect.y - 1);
  const right = Math.min(width - 1, rect.x + rect.width);
  const bottom = Math.min(height - 1, rect.y + rect.height);
  let red = 0;
  let green = 0;
  let blue = 0;
  let count = 0;

  const add = (x: number, y: number) => {
    const index = (y * width + x) * 4;
    red += pixels[index];
    green += pixels[index + 1];
    blue += pixels[index + 2];
    count += 1;
  };

  if (left > right || top > bottom) return "#FFFFFF";

  for (let x = left; x <= right; x += 1) {
    add(x, top);
    if (bottom !== top) add(x, bottom);
  }
  for (let y = top + 1; y < bottom; y += 1) {
    add(left, y);
    if (right !== left) add(right, y);
  }

  return `#${toHex(Math.round(red / count))}${toHex(Math.round(green / count))}${toHex(Math.round(blue / count))}`;
};

/** Pixel edges around a point where the color stops matching, for the ruler. */
export interface PixelSpans {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

/**
 * Walks left, right, up, and down from the pixel under `point` until a pixel differs
 * by more than `tolerance` in any channel. Edges are pixel boundaries inside `bounds`.
 */
export const measureSpans = (
  pixels: Uint8ClampedArray,
  width: number,
  height: number,
  point: PixelPoint,
  bounds: PixelRect,
  tolerance: number,
): PixelSpans | null => {
  const minX = Math.max(0, bounds.x);
  const minY = Math.max(0, bounds.y);
  const maxX = Math.min(width, bounds.x + bounds.width);
  const maxY = Math.min(height, bounds.y + bounds.height);
  const x = Math.floor(point.x);
  const y = Math.floor(point.y);

  if (x < minX || y < minY || x >= maxX || y >= maxY) return null;

  const origin = (y * width + x) * 4;
  const matches = (column: number, row: number) => {
    const index = (row * width + column) * 4;

    return (
      Math.abs(pixels[index] - pixels[origin]) <= tolerance &&
      Math.abs(pixels[index + 1] - pixels[origin + 1]) <= tolerance &&
      Math.abs(pixels[index + 2] - pixels[origin + 2]) <= tolerance
    );
  };

  let left = x;
  while (left > minX && matches(left - 1, y)) left -= 1;
  let right = x + 1;
  while (right < maxX && matches(right, y)) right += 1;
  let top = y;
  while (top > minY && matches(x, top - 1)) top -= 1;
  let bottom = y + 1;
  while (bottom < maxY && matches(x, bottom)) bottom += 1;

  return { bottom, left, right, top };
};
