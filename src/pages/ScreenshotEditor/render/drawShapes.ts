import type { PixelPoint } from "@/utils/pixelRect";
import type {
  CounterShape,
  LineShape,
  PenShape,
  RectShape,
  RulerShape,
  Shape,
} from "../model/shapes";
import type { Canvas2D } from "./drawDocument";
import { drawBlur, drawMagnifier } from "./effects";
import { contrastColor, drawText, TEXT_FONT_FAMILY } from "./text";

/** Tapered arrow with a swept-back head, sized by the stroke width. */
const drawArrow = (context: Canvas2D, shape: LineShape) => {
  const { from, to, strokeWidth } = shape;
  const length = Math.hypot(to.x - from.x, to.y - from.y);
  if (length < 1) return;

  const ux = (to.x - from.x) / length;
  const uy = (to.y - from.y) / length;
  const nx = -uy;
  const ny = ux;
  const headLength = Math.min(length * 0.7, strokeWidth * 3.2 + 10);
  const headHalf = headLength * 0.52;
  const neckHalf = strokeWidth * 0.55;
  const tailHalf = strokeWidth * 0.2;
  const baseX = to.x - ux * headLength;
  const baseY = to.y - uy * headLength;
  const neckX = to.x - ux * headLength * 0.78;
  const neckY = to.y - uy * headLength * 0.78;

  context.beginPath();
  context.moveTo(from.x + nx * tailHalf, from.y + ny * tailHalf);
  context.lineTo(neckX + nx * neckHalf, neckY + ny * neckHalf);
  context.lineTo(baseX + nx * headHalf, baseY + ny * headHalf);
  context.lineTo(to.x, to.y);
  context.lineTo(baseX - nx * headHalf, baseY - ny * headHalf);
  context.lineTo(neckX - nx * neckHalf, neckY - ny * neckHalf);
  context.lineTo(from.x - nx * tailHalf, from.y - ny * tailHalf);
  context.closePath();
  context.fillStyle = shape.color;
  context.strokeStyle = shape.color;
  context.lineJoin = "round";
  context.lineWidth = Math.max(1, strokeWidth * 0.25);
  context.fill();
  context.stroke();
};

const drawLine = (context: Canvas2D, shape: LineShape) => {
  context.beginPath();
  context.moveTo(shape.from.x, shape.from.y);
  context.lineTo(shape.to.x, shape.to.y);
  context.strokeStyle = shape.color;
  context.lineWidth = shape.strokeWidth;
  context.lineCap = "round";
  context.stroke();
};

const drawBox = (context: Canvas2D, shape: RectShape) => {
  context.beginPath();
  if (shape.kind === "oval") {
    context.ellipse(
      shape.x + shape.width / 2,
      shape.y + shape.height / 2,
      shape.width / 2,
      shape.height / 2,
      0,
      0,
      Math.PI * 2,
    );
  } else {
    context.roundRect(
      shape.x,
      shape.y,
      shape.width,
      shape.height,
      shape.strokeWidth * 0.75,
    );
  }

  if (shape.fill) {
    context.fillStyle = shape.color;
    context.fill();
    return;
  }

  context.strokeStyle = shape.color;
  context.lineWidth = shape.strokeWidth;
  context.lineJoin = "round";
  context.stroke();
};

/** Smooths the freehand stroke with quadratic curves through segment midpoints. */
const drawPen = (context: Canvas2D, shape: PenShape) => {
  const { points } = shape;
  if (points.length === 0) return;

  context.strokeStyle = shape.color;
  context.fillStyle = shape.color;
  context.lineWidth = shape.strokeWidth;
  context.lineCap = "round";
  context.lineJoin = "round";

  if (points.length === 1) {
    context.beginPath();
    context.arc(
      points[0].x,
      points[0].y,
      shape.strokeWidth / 2,
      0,
      Math.PI * 2,
    );
    context.fill();
    return;
  }

  context.beginPath();
  context.moveTo(points[0].x, points[0].y);
  for (let index = 1; index < points.length - 1; index += 1) {
    const current = points[index];
    const next = points[index + 1];
    context.quadraticCurveTo(
      current.x,
      current.y,
      (current.x + next.x) / 2,
      (current.y + next.y) / 2,
    );
  }
  const last = points[points.length - 1];
  context.lineTo(last.x, last.y);
  context.stroke();
};

const drawCounter = (context: Canvas2D, shape: CounterShape) => {
  const { radius, x, y } = shape;

  context.save();
  context.shadowColor = "rgba(0, 0, 0, 0.25)";
  context.shadowBlur = radius * 0.3 * Math.abs(context.getTransform().a);
  context.beginPath();
  context.arc(x, y, radius, 0, Math.PI * 2);
  context.fillStyle = shape.color;
  context.fill();
  context.restore();

  context.beginPath();
  context.arc(x, y, radius, 0, Math.PI * 2);
  context.strokeStyle = contrastColor(shape.color);
  context.globalAlpha = 0.9;
  context.lineWidth = Math.max(1, radius * 0.12);
  context.stroke();
  context.globalAlpha = 1;

  const label = String(shape.value);
  const fontSize =
    radius * (label.length > 2 ? 0.8 : label.length > 1 ? 0.95 : 1.1);
  context.font = `700 ${fontSize}px ${TEXT_FONT_FAMILY}`;
  context.fillStyle = contrastColor(shape.color);
  context.textAlign = "center";
  context.textBaseline = "middle";
  context.fillText(label, x, y + fontSize * 0.04);
};

const rulerLabel = (from: PixelPoint, to: PixelPoint) => {
  const dx = Math.abs(to.x - from.x);
  const dy = Math.abs(to.y - from.y);
  if (dy < 0.5) return `${Math.round(dx)} px`;
  if (dx < 0.5) return `${Math.round(dy)} px`;

  return `${Math.round(Math.hypot(dx, dy))} px`;
};

/** Measurement line with end ticks and a length label, like a design tool guide. */
export const drawRuler = (
  context: Canvas2D,
  shape: Pick<RulerShape, "color" | "fontSize" | "from" | "strokeWidth" | "to">,
) => {
  const { color, fontSize, from, strokeWidth, to } = shape;
  const length = Math.hypot(to.x - from.x, to.y - from.y);
  if (length < 1) return;

  const nx = -(to.y - from.y) / length;
  const ny = (to.x - from.x) / length;
  const tick = strokeWidth * 3;

  context.strokeStyle = color;
  context.lineWidth = strokeWidth;
  context.lineCap = "butt";
  context.beginPath();
  context.moveTo(from.x, from.y);
  context.lineTo(to.x, to.y);
  context.moveTo(from.x + nx * tick, from.y + ny * tick);
  context.lineTo(from.x - nx * tick, from.y - ny * tick);
  context.moveTo(to.x + nx * tick, to.y + ny * tick);
  context.lineTo(to.x - nx * tick, to.y - ny * tick);
  context.stroke();

  const label = rulerLabel(from, to);
  context.font = `600 ${fontSize}px ${TEXT_FONT_FAMILY}`;
  const paddingX = fontSize * 0.45;
  const width = context.measureText(label).width + paddingX * 2;
  const height = fontSize * 1.6;
  const centerX = (from.x + to.x) / 2;
  const centerY = (from.y + to.y) / 2;

  context.fillStyle = color;
  context.beginPath();
  context.roundRect(
    centerX - width / 2,
    centerY - height / 2,
    width,
    height,
    height / 2,
  );
  context.fill();
  context.fillStyle = contrastColor(color);
  context.textAlign = "center";
  context.textBaseline = "middle";
  context.fillText(label, centerX, centerY + fontSize * 0.04);
};

/** Draws one annotation in image coordinates under the current transform. */
export const drawShape = (
  context: Canvas2D,
  bitmap: ImageBitmap,
  shape: Shape,
) => {
  context.save();

  switch (shape.kind) {
    case "arrow":
      drawArrow(context, shape);
      break;
    case "line":
      drawLine(context, shape);
      break;
    case "rect":
    case "oval":
      drawBox(context, shape);
      break;
    case "pen":
      drawPen(context, shape);
      break;
    case "highlighter":
      context.fillStyle = shape.color;
      context.beginPath();
      context.roundRect(
        shape.x,
        shape.y,
        shape.width,
        shape.height,
        Math.min(3, shape.width / 2, shape.height / 2),
      );
      // A light ink wash stays visible on dark captures; multiply keeps dark
      // lettering readable under the vivid marker color on light captures.
      context.globalAlpha = (shape.opacity / 100) * 0.25;
      context.fill();
      context.globalCompositeOperation = "multiply";
      context.globalAlpha = shape.opacity / 100;
      context.fill();
      break;
    case "counter":
      drawCounter(context, shape);
      break;
    case "ruler":
      drawRuler(context, shape);
      break;
    case "text":
      drawText(context, shape);
      break;
    case "image":
      context.drawImage(
        shape.bitmap,
        shape.x,
        shape.y,
        shape.width,
        shape.height,
      );
      break;
    case "blur":
      drawBlur(context, bitmap, shape);
      break;
    case "magnifier":
      drawMagnifier(context, bitmap, shape);
      break;
    case "spotlight":
      // Spotlights are combined into one shade layer by the document renderer.
      break;
  }

  context.restore();
};
