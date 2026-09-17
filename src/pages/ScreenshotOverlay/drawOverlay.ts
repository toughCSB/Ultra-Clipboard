import type { PixelPoint, PixelRect } from "@/utils/pixelRect";

export interface OverlayMarks {
  bitmap: ImageBitmap;
  /** Device pixels per CSS pixel for this monitor. */
  ratio: number;
  pointer: PixelPoint | null;
  highlight: PixelRect | null;
  showCrosshair: boolean;
}

const SHADE = "rgba(0, 0, 0, 0.4)";
const ACCENT = "#2dd4bf";
const LOUPE_CSS = 120;
const LOUPE_SOURCE = 15;
const LOUPE_OFFSET_CSS = 24;
const FONT = 'Pretendard, "Malgun Gothic", system-ui, sans-serif';

/** Draws shade, crosshair, selection, and loupe over the frozen frame. */
export const drawOverlay = (
  context: CanvasRenderingContext2D,
  marks: OverlayMarks,
) => {
  const { bitmap, highlight, pointer, ratio, showCrosshair } = marks;
  const { width, height } = context.canvas;

  context.clearRect(0, 0, width, height);
  context.save();
  context.beginPath();
  context.rect(0, 0, width, height);
  if (highlight) {
    context.rect(highlight.x, highlight.y, highlight.width, highlight.height);
  }
  context.fillStyle = SHADE;
  context.fill("evenodd");
  context.restore();

  if (highlight) {
    const line = Math.max(1, Math.round(ratio * 2));

    context.strokeStyle = ACCENT;
    context.lineWidth = line;
    context.strokeRect(
      highlight.x - line / 2,
      highlight.y - line / 2,
      highlight.width + line,
      highlight.height + line,
    );
    drawBadge(
      context,
      `${highlight.width} × ${highlight.height}`,
      highlight,
      ratio,
    );
  }

  if (!pointer) return;

  if (showCrosshair) {
    drawCrosshair(context, pointer);
  }

  drawLoupe(context, bitmap, pointer, ratio);
};

const drawCrosshair = (
  context: CanvasRenderingContext2D,
  pointer: PixelPoint,
) => {
  const { width, height } = context.canvas;
  const x = Math.floor(pointer.x) + 0.5;
  const y = Math.floor(pointer.y) + 0.5;

  context.beginPath();
  context.moveTo(x, 0);
  context.lineTo(x, height);
  context.moveTo(0, y);
  context.lineTo(width, y);
  context.lineWidth = 3;
  context.strokeStyle = "rgba(0, 0, 0, 0.45)";
  context.stroke();
  context.lineWidth = 1;
  context.strokeStyle = "rgba(255, 255, 255, 0.9)";
  context.stroke();
};

const drawLoupe = (
  context: CanvasRenderingContext2D,
  bitmap: ImageBitmap,
  pointer: PixelPoint,
  ratio: number,
) => {
  const { width, height } = context.canvas;
  const size = Math.round(LOUPE_CSS * ratio);
  const cell = size / LOUPE_SOURCE;
  const offset = Math.round(LOUPE_OFFSET_CSS * ratio);
  const captionHeight = Math.round(22 * ratio);
  const px = Math.floor(pointer.x);
  const py = Math.floor(pointer.y);
  const right = pointer.x + offset + size <= width;
  const below = pointer.y + offset + size + captionHeight <= height;
  const left = right ? pointer.x + offset : pointer.x - offset - size;
  const top = below
    ? pointer.y + offset
    : pointer.y - offset - size - captionHeight;
  const half = Math.floor(LOUPE_SOURCE / 2);
  const sourceLeft = Math.max(0, px - half);
  const sourceTop = Math.max(0, py - half);
  const sourceRight = Math.min(bitmap.width, px + half + 1);
  const sourceBottom = Math.min(bitmap.height, py + half + 1);

  context.save();
  context.fillStyle = "#000000";
  context.fillRect(left, top, size, size);
  context.imageSmoothingEnabled = false;
  if (sourceRight > sourceLeft && sourceBottom > sourceTop) {
    context.drawImage(
      bitmap,
      sourceLeft,
      sourceTop,
      sourceRight - sourceLeft,
      sourceBottom - sourceTop,
      left + (sourceLeft - (px - half)) * cell,
      top + (sourceTop - (py - half)) * cell,
      (sourceRight - sourceLeft) * cell,
      (sourceBottom - sourceTop) * cell,
    );
  }

  context.beginPath();
  for (let index = 1; index < LOUPE_SOURCE; index += 1) {
    const offsetLine = Math.round(index * cell) + 0.5;
    context.moveTo(left + offsetLine, top);
    context.lineTo(left + offsetLine, top + size);
    context.moveTo(left, top + offsetLine);
    context.lineTo(left + size, top + offsetLine);
  }
  context.lineWidth = 1;
  context.strokeStyle = "rgba(0, 0, 0, 0.18)";
  context.stroke();

  context.lineWidth = Math.max(1, Math.round(ratio * 1.5));
  context.strokeStyle = ACCENT;
  context.strokeRect(left + half * cell, top + half * cell, cell, cell);
  context.strokeStyle = "#ffffff";
  context.strokeRect(left, top, size, size);
  context.restore();

  const caption = `${px}, ${py}`;
  context.font = `600 ${Math.round(12 * ratio)}px ${FONT}`;
  context.fillStyle = "rgba(15, 23, 42, 0.85)";
  context.fillRect(left, top + size, size, captionHeight);
  context.fillStyle = "#ffffff";
  context.textAlign = "center";
  context.textBaseline = "middle";
  context.fillText(caption, left + size / 2, top + size + captionHeight / 2);
  context.textAlign = "start";
};

const drawBadge = (
  context: CanvasRenderingContext2D,
  label: string,
  rect: PixelRect,
  ratio: number,
) => {
  const fontSize = Math.round(12 * ratio);
  const paddingX = Math.round(7 * ratio);
  const badgeHeight = Math.round(22 * ratio);
  const gap = Math.round(6 * ratio);

  context.font = `600 ${fontSize}px ${FONT}`;
  const badgeWidth = context.measureText(label).width + paddingX * 2;
  const x = Math.min(Math.max(0, rect.x), context.canvas.width - badgeWidth);
  const above = rect.y - badgeHeight - gap;
  const y =
    above >= 0
      ? above
      : Math.min(rect.y + gap, context.canvas.height - badgeHeight);

  context.fillStyle = "rgba(15, 23, 42, 0.85)";
  context.fillRect(x, y, badgeWidth, badgeHeight);
  context.fillStyle = "#ffffff";
  context.textBaseline = "middle";
  context.fillText(label, x + paddingX, y + badgeHeight / 2);
};
