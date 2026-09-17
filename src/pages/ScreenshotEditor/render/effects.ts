import type { PixelRect } from "@/utils/pixelRect";
import type {
  BlurShape,
  MagnifierShape,
  SpotlightShape,
} from "../model/shapes";
import type { Canvas2D } from "./drawDocument";

const SPOTLIGHT_SHADE = "rgba(0, 0, 0, 0.55)";

/** Effect tiles are immutable per shape object, so they are rendered once and reused. */
const effectCache = new WeakMap<BlurShape, OffscreenCanvas>();
let spotlightCache: { key: string; layer: OffscreenCanvas } | null = null;

const createContext = (width: number, height: number) => {
  const canvas = new OffscreenCanvas(Math.max(1, width), Math.max(1, height));
  const context = canvas.getContext("2d");
  if (!context) throw new Error("2d canvas context is unavailable");

  return { canvas, context };
};

/**
 * Blur without `CanvasRenderingContext2D.filter`, which WebKit lacks: repeatedly
 * shrinking and enlarging with smoothing approximates a gaussian blur.
 */
const renderBlur = (
  bitmap: ImageBitmap,
  box: PixelRect,
  strength: number,
): OffscreenCanvas => {
  const { canvas, context } = createContext(box.width, box.height);
  const margin = Math.ceil(strength * 2);
  const sourceX = box.x - margin;
  const sourceY = box.y - margin;
  const sourceWidth = box.width + margin * 2;
  const sourceHeight = box.height + margin * 2;
  const factor = Math.max(2, strength / 2);
  const smallWidth = Math.max(1, Math.round(sourceWidth / factor));
  const smallHeight = Math.max(1, Math.round(sourceHeight / factor));
  const small = createContext(smallWidth, smallHeight);
  const middle = createContext(
    Math.max(1, Math.round(sourceWidth / Math.sqrt(factor))),
    Math.max(1, Math.round(sourceHeight / Math.sqrt(factor))),
  );

  small.context.imageSmoothingEnabled = true;
  small.context.imageSmoothingQuality = "high";
  small.context.drawImage(
    bitmap,
    sourceX,
    sourceY,
    sourceWidth,
    sourceHeight,
    0,
    0,
    smallWidth,
    smallHeight,
  );

  middle.context.imageSmoothingEnabled = true;
  middle.context.imageSmoothingQuality = "high";
  middle.context.drawImage(
    small.canvas,
    0,
    0,
    middle.canvas.width,
    middle.canvas.height,
  );

  context.imageSmoothingEnabled = true;
  context.imageSmoothingQuality = "high";
  context.drawImage(
    middle.canvas,
    0,
    0,
    middle.canvas.width,
    middle.canvas.height,
    -margin,
    -margin,
    sourceWidth,
    sourceHeight,
  );

  return canvas;
};

const renderPixelate = (
  bitmap: ImageBitmap,
  box: PixelRect,
  block: number,
): OffscreenCanvas => {
  const { canvas, context } = createContext(box.width, box.height);
  const cell = Math.max(2, Math.round(block));
  const small = createContext(
    Math.max(1, Math.ceil(box.width / cell)),
    Math.max(1, Math.ceil(box.height / cell)),
  );

  small.context.imageSmoothingEnabled = true;
  small.context.imageSmoothingQuality = "high";
  small.context.drawImage(
    bitmap,
    box.x,
    box.y,
    small.canvas.width * cell,
    small.canvas.height * cell,
    0,
    0,
    small.canvas.width,
    small.canvas.height,
  );

  context.imageSmoothingEnabled = false;
  context.drawImage(
    small.canvas,
    0,
    0,
    small.canvas.width * cell,
    small.canvas.height * cell,
  );

  return canvas;
};

const effectTile = (bitmap: ImageBitmap, shape: BlurShape) => {
  const cached = effectCache.get(shape);
  if (cached) return cached;

  const box = {
    height: Math.max(1, Math.round(shape.height)),
    width: Math.max(1, Math.round(shape.width)),
    x: Math.round(shape.x),
    y: Math.round(shape.y),
  };
  const tile =
    shape.mode === "pixelate"
      ? renderPixelate(bitmap, box, shape.strength)
      : renderBlur(bitmap, box, shape.strength);

  effectCache.set(shape, tile);

  return tile;
};

/** Hides the pixels under a blur shape. Only the original capture is sampled. */
export const drawBlur = (
  context: Canvas2D,
  bitmap: ImageBitmap,
  shape: BlurShape,
) => {
  if (shape.width < 1 || shape.height < 1) return;

  if (shape.mode === "erase") {
    context.fillStyle = shape.fillColor;
    context.fillRect(shape.x, shape.y, shape.width, shape.height);
    return;
  }

  context.drawImage(
    effectTile(bitmap, shape),
    Math.round(shape.x),
    Math.round(shape.y),
  );
};

/** Darkens `area` everywhere except inside the spotlight shapes. */
export const drawSpotlights = (
  context: Canvas2D,
  spotlights: readonly SpotlightShape[],
  area: PixelRect,
) => {
  if (spotlights.length === 0) return;

  const key = [
    area.x,
    area.y,
    area.width,
    area.height,
    ...spotlights.map((spot) => {
      return `${spot.x},${spot.y},${spot.width},${spot.height},${spot.oval}`;
    }),
  ].join("|");

  if (spotlightCache?.key !== key) {
    const { canvas, context: layer } = createContext(area.width, area.height);

    layer.fillStyle = SPOTLIGHT_SHADE;
    layer.fillRect(0, 0, area.width, area.height);
    layer.globalCompositeOperation = "destination-out";
    layer.fillStyle = "#000000";

    for (const spot of spotlights) {
      layer.beginPath();
      if (spot.oval) {
        layer.ellipse(
          spot.x - area.x + spot.width / 2,
          spot.y - area.y + spot.height / 2,
          spot.width / 2,
          spot.height / 2,
          0,
          0,
          Math.PI * 2,
        );
      } else {
        layer.rect(spot.x - area.x, spot.y - area.y, spot.width, spot.height);
      }
      layer.fill();
    }

    spotlightCache = { key, layer: canvas };
  }

  context.drawImage(spotlightCache.layer, area.x, area.y);
};

/** Draws a circular lens that shows the capture around its source point enlarged. */
export const drawMagnifier = (
  context: Canvas2D,
  bitmap: ImageBitmap,
  shape: MagnifierShape,
) => {
  const radius = Math.min(shape.width, shape.height) / 2;
  if (radius < 1) return;

  const centerX = shape.x + shape.width / 2;
  const centerY = shape.y + shape.height / 2;
  const sourceSize = (radius * 2) / shape.zoom;
  const ring = shape.strokeWidth;

  context.save();
  context.shadowColor = "rgba(0, 0, 0, 0.35)";
  context.shadowBlur = ring * 3 * Math.abs(context.getTransform().a);
  context.fillStyle = "#FFFFFF";
  context.beginPath();
  context.arc(centerX, centerY, radius + ring, 0, Math.PI * 2);
  context.fill();
  context.restore();

  context.save();
  context.beginPath();
  context.arc(centerX, centerY, radius, 0, Math.PI * 2);
  context.clip();
  context.drawImage(
    bitmap,
    shape.sourceX - sourceSize / 2,
    shape.sourceY - sourceSize / 2,
    sourceSize,
    sourceSize,
    centerX - radius,
    centerY - radius,
    radius * 2,
    radius * 2,
  );
  context.restore();

  context.save();
  context.strokeStyle = "rgba(0, 0, 0, 0.25)";
  context.lineWidth = Math.max(1, ring / 3);
  context.beginPath();
  context.arc(centerX, centerY, radius + ring, 0, Math.PI * 2);
  context.stroke();
  context.restore();
};
