import type { PixelRect } from "@/utils/pixelRect";
import { documentBounds } from "../model/document";
import type { PixelSpans } from "../model/pixels";
import {
  type Box,
  type Shape,
  shapeBounds,
  shapeHandles,
} from "../model/shapes";
import { PIXEL_GRID_ZOOM, type Viewport } from "../model/viewport";
import { drawDocument, type EditorScene } from "./drawDocument";
import { drawRuler } from "./drawShapes";

export interface EditorView {
  /** Base and overlay colors of the transparency checkerboard. */
  checker: readonly [string, string];
  /** Live ruler measurement around the pointer. */
  measure: PixelSpans | null;
  ratio: number;
  scene: EditorScene;
  selectedShape: Shape | null;
  selection: PixelRect | null;
  viewport: Viewport;
}

const CHECKER_CELL_CSS = 8;
const ACCENT_COLOR = "#14b8a6";
const MEASURE_COLOR = "#FF2D55";
const SELECTION_SHADE = "rgba(0, 0, 0, 0.45)";
const HANDLE_CSS = 7;
const SHAPE_OUTLINE_GAP_CSS = 4;

const checkerPatterns = new Map<string, CanvasPattern>();

/** Paints the editor canvas: checkerboard, document, pixel grid, and editing chrome. */
export const drawView = (
  context: CanvasRenderingContext2D,
  view: EditorView,
) => {
  const { checker, measure, ratio, scene, selectedShape, selection, viewport } =
    view;
  const { width, height } = context.canvas;
  const bounds = documentBounds(scene.document);

  context.setTransform(1, 0, 0, 1, 0, 0);
  context.clearRect(0, 0, width, height);
  context.fillStyle = checkerPattern(context, ratio, checker);
  context.fillRect(0, 0, width, height);

  context.setTransform(
    viewport.zoom,
    0,
    0,
    viewport.zoom,
    viewport.offsetX,
    viewport.offsetY,
  );
  context.imageSmoothingEnabled = viewport.zoom < 1;
  context.imageSmoothingQuality = "high";
  drawDocument(context, scene, bounds);

  if (measure) {
    drawMeasure(context, measure, viewport, ratio);
  }

  context.setTransform(1, 0, 0, 1, 0, 0);

  if (viewport.zoom >= PIXEL_GRID_ZOOM) {
    drawPixelGrid(context, bounds, viewport);
  }

  if (selection) {
    drawSelection(context, bounds, selection, viewport, ratio);
  }

  if (selectedShape) {
    drawShapeChrome(context, selectedShape, viewport, ratio);
  }
};

const checkerPattern = (
  context: CanvasRenderingContext2D,
  ratio: number,
  checker: readonly [string, string],
) => {
  const cell = Math.max(1, Math.round(CHECKER_CELL_CSS * ratio));
  const [base, alternate] = checker;
  const key = `${cell}:${base}:${alternate}`;
  const cached = checkerPatterns.get(key);
  if (cached) return cached;

  const tile = new OffscreenCanvas(cell * 2, cell * 2);
  const tileContext = tile.getContext("2d");

  if (tileContext) {
    tileContext.fillStyle = base;
    tileContext.fillRect(0, 0, cell * 2, cell * 2);
    tileContext.fillStyle = alternate;
    tileContext.fillRect(0, 0, cell, cell);
    tileContext.fillRect(cell, cell, cell, cell);
  }

  const pattern = context.createPattern(tile, "repeat");
  if (!pattern) return base;

  checkerPatterns.set(key, pattern);

  return pattern;
};

const toDevice = (viewport: Viewport, rect: Box) => {
  return {
    height: rect.height * viewport.zoom,
    width: rect.width * viewport.zoom,
    x: rect.x * viewport.zoom + viewport.offsetX,
    y: rect.y * viewport.zoom + viewport.offsetY,
  };
};

const drawPixelGrid = (
  context: CanvasRenderingContext2D,
  bounds: PixelRect,
  viewport: Viewport,
) => {
  const area = toDevice(viewport, bounds);
  const { width, height } = context.canvas;
  const firstColumn = Math.max(
    bounds.x,
    Math.floor(-viewport.offsetX / viewport.zoom),
  );
  const lastColumn = Math.min(
    bounds.x + bounds.width,
    Math.ceil((width - viewport.offsetX) / viewport.zoom),
  );
  const firstRow = Math.max(
    bounds.y,
    Math.floor(-viewport.offsetY / viewport.zoom),
  );
  const lastRow = Math.min(
    bounds.y + bounds.height,
    Math.ceil((height - viewport.offsetY) / viewport.zoom),
  );

  context.beginPath();
  for (let column = firstColumn; column <= lastColumn; column += 1) {
    const x = Math.round(column * viewport.zoom + viewport.offsetX) + 0.5;
    context.moveTo(x, Math.max(0, area.y));
    context.lineTo(x, Math.min(height, area.y + area.height));
  }
  for (let row = firstRow; row <= lastRow; row += 1) {
    const y = Math.round(row * viewport.zoom + viewport.offsetY) + 0.5;
    context.moveTo(Math.max(0, area.x), y);
    context.lineTo(Math.min(width, area.x + area.width), y);
  }
  context.strokeStyle = "rgba(128, 128, 128, 0.35)";
  context.lineWidth = 1;
  context.stroke();
};

const drawSelection = (
  context: CanvasRenderingContext2D,
  bounds: PixelRect,
  selection: PixelRect,
  viewport: Viewport,
  ratio: number,
) => {
  const area = toDevice(viewport, bounds);
  const rect = toDevice(viewport, selection);

  context.save();
  context.beginPath();
  context.rect(area.x, area.y, area.width, area.height);
  context.rect(rect.x, rect.y, rect.width, rect.height);
  context.fillStyle = SELECTION_SHADE;
  context.fill("evenodd");
  context.restore();

  context.strokeStyle = ACCENT_COLOR;
  context.lineWidth = Math.max(1, Math.round(ratio * 1.5));
  context.strokeRect(rect.x, rect.y, rect.width, rect.height);

  const handle = Math.round(HANDLE_CSS * ratio);
  const handlePoints = [
    [rect.x, rect.y],
    [rect.x + rect.width / 2, rect.y],
    [rect.x + rect.width, rect.y],
    [rect.x + rect.width, rect.y + rect.height / 2],
    [rect.x + rect.width, rect.y + rect.height],
    [rect.x + rect.width / 2, rect.y + rect.height],
    [rect.x, rect.y + rect.height],
    [rect.x, rect.y + rect.height / 2],
  ];

  context.fillStyle = "#ffffff";
  context.lineWidth = Math.max(1, Math.round(ratio));
  for (const [x, y] of handlePoints) {
    context.fillRect(x - handle / 2, y - handle / 2, handle, handle);
    context.strokeRect(x - handle / 2, y - handle / 2, handle, handle);
  }

  drawSizeBadge(
    context,
    `${selection.width} × ${selection.height}`,
    rect,
    ratio,
  );
};

/** Dashed outline and handles around the selected annotation. */
const drawShapeChrome = (
  context: CanvasRenderingContext2D,
  shape: Shape,
  viewport: Viewport,
  ratio: number,
) => {
  const handles = shapeHandles(shape);
  const handleSize = Math.round(HANDLE_CSS * ratio);

  context.save();
  context.strokeStyle = ACCENT_COLOR;
  context.lineWidth = Math.max(1, Math.round(ratio));

  if (handles.length === 0 || shape.kind === "magnifier") {
    const gap = SHAPE_OUTLINE_GAP_CSS * ratio;
    const outline = toDevice(viewport, shapeBounds(shape));

    context.setLineDash([4 * ratio, 3 * ratio]);
    context.strokeRect(
      outline.x - gap,
      outline.y - gap,
      outline.width + gap * 2,
      outline.height + gap * 2,
    );
    context.setLineDash([]);
  }

  if (shape.kind === "magnifier") {
    const source = {
      x: shape.sourceX * viewport.zoom + viewport.offsetX,
      y: shape.sourceY * viewport.zoom + viewport.offsetY,
    };
    const sourceRadius =
      (Math.min(shape.width, shape.height) / 2 / shape.zoom) * viewport.zoom;

    context.setLineDash([4 * ratio, 3 * ratio]);
    context.beginPath();
    context.arc(source.x, source.y, Math.max(2, sourceRadius), 0, Math.PI * 2);
    context.stroke();
    context.setLineDash([]);
  }

  context.fillStyle = "#ffffff";
  for (const { handle, point } of handles) {
    const x = point.x * viewport.zoom + viewport.offsetX;
    const y = point.y * viewport.zoom + viewport.offsetY;

    context.beginPath();
    if (handle === "start" || handle === "end") {
      context.arc(x, y, handleSize / 2 + ratio, 0, Math.PI * 2);
    } else {
      context.rect(
        x - handleSize / 2,
        y - handleSize / 2,
        handleSize,
        handleSize,
      );
    }
    context.fill();
    context.stroke();
  }

  context.restore();
};

/** Shows the live ruler spans in screen-sized strokes under the image transform. */
const drawMeasure = (
  context: CanvasRenderingContext2D,
  spans: PixelSpans,
  viewport: Viewport,
  ratio: number,
) => {
  const strokeWidth = (1.5 * ratio) / viewport.zoom;
  const fontSize = (11 * ratio) / viewport.zoom;
  const centerY = spans.top + (spans.bottom - spans.top) / 2;
  const centerX = spans.left + (spans.right - spans.left) / 2;

  context.save();
  drawRuler(context, {
    color: MEASURE_COLOR,
    fontSize,
    from: { x: spans.left, y: centerY },
    strokeWidth,
    to: { x: spans.right, y: centerY },
  });
  drawRuler(context, {
    color: MEASURE_COLOR,
    fontSize,
    from: { x: centerX, y: spans.top },
    strokeWidth,
    to: { x: centerX, y: spans.bottom },
  });
  context.restore();
};

const drawSizeBadge = (
  context: CanvasRenderingContext2D,
  label: string,
  rect: { x: number; y: number; width: number; height: number },
  ratio: number,
) => {
  const fontSize = Math.round(12 * ratio);
  const paddingX = Math.round(6 * ratio);
  const badgeHeight = Math.round(20 * ratio);

  context.font = `600 ${fontSize}px Pretendard, "Malgun Gothic", system-ui, sans-serif`;
  const badgeWidth = context.measureText(label).width + paddingX * 2;
  const gap = Math.round(6 * ratio);
  const above = rect.y - badgeHeight - gap;
  const x = Math.max(0, Math.min(rect.x, context.canvas.width - badgeWidth));
  const y = above >= 0 ? above : rect.y + gap;

  context.fillStyle = "rgba(15, 23, 42, 0.85)";
  context.fillRect(x, y, badgeWidth, badgeHeight);
  context.fillStyle = "#ffffff";
  context.textBaseline = "middle";
  context.fillText(label, x + paddingX, y + badgeHeight / 2);
};
