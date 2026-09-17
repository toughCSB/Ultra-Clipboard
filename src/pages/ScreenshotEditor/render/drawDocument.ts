import { intersectRects, type PixelRect } from "@/utils/pixelRect";
import {
  type Backdrop,
  type BackdropBackground,
  documentBounds,
  type EditorDocument,
} from "../model/document";
import type { SpotlightShape } from "../model/shapes";
import { drawShape } from "./drawShapes";
import { drawSpotlights } from "./effects";

export type Canvas2D =
  | CanvasRenderingContext2D
  | OffscreenCanvasRenderingContext2D;

export type { EditorDocument } from "../model/document";

export interface EditorScene {
  bitmap: ImageBitmap;
  document: EditorDocument;
  /** Shape left out while it is edited in place, such as text under its editor. */
  hiddenShapeId?: string | null;
}

export const BACKDROP_GRADIENTS: Record<BackdropBackground, readonly string[]> =
  {
    forest: ["#11998E", "#38EF7D"],
    graphite: ["#4B5563", "#111827"],
    light: ["#F8FAFC", "#E2E8F0"],
    ocean: ["#2BC0E4", "#6A82FB"],
    sunset: ["#FFB88C", "#FF6A88", "#DE6262"],
    transparent: [],
  };

const drawBackdropBackground = (
  context: Canvas2D,
  backdrop: Backdrop,
  bounds: PixelRect,
) => {
  const stops = BACKDROP_GRADIENTS[backdrop.background];
  if (stops.length === 0) return;

  const gradient = context.createLinearGradient(
    bounds.x,
    bounds.y,
    bounds.x + bounds.width,
    bounds.y + bounds.height,
  );
  stops.forEach((color, index) => {
    gradient.addColorStop(index / Math.max(1, stops.length - 1), color);
  });

  context.fillStyle = gradient;
  context.fillRect(bounds.x, bounds.y, bounds.width, bounds.height);
};

const imagePath = (context: Canvas2D, crop: PixelRect, radius: number) => {
  context.beginPath();
  if (radius > 0) {
    context.roundRect(crop.x, crop.y, crop.width, crop.height, radius);
  } else {
    context.rect(crop.x, crop.y, crop.width, crop.height);
  }
};

/**
 * Draws the document in image coordinates under the context's current transform.
 * The editor view and the export share this function, so what the editor shows
 * is exactly what gets copied or saved.
 */
export const drawDocument = (
  context: Canvas2D,
  scene: EditorScene,
  clip: PixelRect,
) => {
  const { bitmap, document, hiddenShapeId } = scene;
  const { backdrop, crop, shapes } = document;
  const bounds = documentBounds(document);
  const area = intersectRects(bounds, clip);
  if (!area) return;

  const scale = Math.abs(context.getTransform().a) || 1;
  const radius = backdrop ? backdrop.radius : 0;
  const visible = shapes.filter((shape) => shape.id !== hiddenShapeId);
  const spotlights = visible.filter(
    (shape): shape is SpotlightShape => shape.kind === "spotlight",
  );

  context.save();
  context.beginPath();
  context.rect(area.x, area.y, area.width, area.height);
  context.clip();

  if (backdrop) {
    drawBackdropBackground(context, backdrop, bounds);

    if (backdrop.shadow) {
      context.save();
      context.shadowColor = "rgba(0, 0, 0, 0.35)";
      context.shadowBlur = backdrop.padding * 0.35 * scale;
      context.shadowOffsetY = backdrop.padding * 0.08 * scale;
      context.fillStyle = "#000000";
      imagePath(context, crop, radius);
      context.fill();
      context.restore();
    }
  }

  // The capture and its raster effects stay inside the (rounded) image frame.
  context.save();
  imagePath(context, crop, radius);
  context.clip();
  context.drawImage(
    bitmap,
    crop.x,
    crop.y,
    crop.width,
    crop.height,
    crop.x,
    crop.y,
    crop.width,
    crop.height,
  );
  for (const shape of visible) {
    if (shape.kind === "blur") drawShape(context, bitmap, shape);
  }
  drawSpotlights(context, spotlights, crop);
  context.restore();

  for (const shape of visible) {
    if (shape.kind !== "blur" && shape.kind !== "spotlight") {
      drawShape(context, bitmap, shape);
    }
  }

  context.restore();
};
