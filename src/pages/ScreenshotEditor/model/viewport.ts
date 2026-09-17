import type { PixelPoint, PixelRect } from "../../../utils/pixelRect";

/**
 * Maps image pixels to canvas device pixels: `device = image * zoom + offset`.
 * A zoom of 1 shows one image pixel per device pixel, the size it had on screen.
 */
export interface Viewport {
  zoom: number;
  offsetX: number;
  offsetY: number;
}

export const MIN_ZOOM = 0.05;
export const MAX_ZOOM = 32;
export const ZOOM_STEP = 1.25;
/** Pixel grid lines appear from this zoom level. */
export const PIXEL_GRID_ZOOM = 8;

const clamp = (value: number, min: number, max: number) => {
  return Math.min(Math.max(value, min), max);
};

export const clampZoom = (zoom: number) => {
  return clamp(zoom, MIN_ZOOM, MAX_ZOOM);
};

/** Centers `content` at `zoom` inside a view of the given device size. */
export const centerViewport = (
  content: PixelRect,
  viewWidth: number,
  viewHeight: number,
  zoom: number,
): Viewport => {
  const nextZoom = clampZoom(zoom);

  return {
    offsetX: Math.round(
      (viewWidth - content.width * nextZoom) / 2 - content.x * nextZoom,
    ),
    offsetY: Math.round(
      (viewHeight - content.height * nextZoom) / 2 - content.y * nextZoom,
    ),
    zoom: nextZoom,
  };
};

/** Shows the whole content with a margin, never enlarging past actual size. */
export const fitViewport = (
  content: PixelRect,
  viewWidth: number,
  viewHeight: number,
  margin: number,
): Viewport => {
  const availableWidth = Math.max(1, viewWidth - margin * 2);
  const availableHeight = Math.max(1, viewHeight - margin * 2);
  const zoom = Math.min(
    1,
    availableWidth / content.width,
    availableHeight / content.height,
  );

  return centerViewport(content, viewWidth, viewHeight, zoom);
};

/** Zooms while keeping the image point under the anchor fixed on screen. */
export const zoomViewportAt = (
  viewport: Viewport,
  zoom: number,
  anchorX: number,
  anchorY: number,
): Viewport => {
  const nextZoom = clampZoom(zoom);
  const imageX = (anchorX - viewport.offsetX) / viewport.zoom;
  const imageY = (anchorY - viewport.offsetY) / viewport.zoom;

  return {
    offsetX: anchorX - imageX * nextZoom,
    offsetY: anchorY - imageY * nextZoom,
    zoom: nextZoom,
  };
};

export const panViewport = (
  viewport: Viewport,
  dx: number,
  dy: number,
): Viewport => {
  return {
    ...viewport,
    offsetX: viewport.offsetX + dx,
    offsetY: viewport.offsetY + dy,
  };
};

/** Keeps at least `visible` device pixels of the content inside the view. */
export const clampViewport = (
  viewport: Viewport,
  content: PixelRect,
  viewWidth: number,
  viewHeight: number,
  visible: number,
): Viewport => {
  const left = content.x * viewport.zoom;
  const top = content.y * viewport.zoom;
  const width = content.width * viewport.zoom;
  const height = content.height * viewport.zoom;
  const keepX = Math.min(visible, width);
  const keepY = Math.min(visible, height);

  return {
    ...viewport,
    offsetX: clamp(
      viewport.offsetX,
      keepX - left - width,
      viewWidth - keepX - left,
    ),
    offsetY: clamp(
      viewport.offsetY,
      keepY - top - height,
      viewHeight - keepY - top,
    ),
  };
};

export const toImagePoint = (
  viewport: Viewport,
  deviceX: number,
  deviceY: number,
): PixelPoint => {
  return {
    x: (deviceX - viewport.offsetX) / viewport.zoom,
    y: (deviceY - viewport.offsetY) / viewport.zoom,
  };
};
