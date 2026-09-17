import type { TextShape, TextStyle } from "../model/shapes";
import type { Canvas2D } from "./drawDocument";

export const TEXT_FONT_FAMILY =
  'Pretendard, "Malgun Gothic", "Apple SD Gothic Neo", system-ui, sans-serif';
export const TEXT_LINE_HEIGHT = 1.25;

let measureContext: OffscreenCanvasRenderingContext2D | null = null;

export const textFont = (fontSize: number) => {
  return `600 ${fontSize}px ${TEXT_FONT_FAMILY}`;
};

/** Inner padding between the text box edge and its glyphs. */
export const textPadding = (fontSize: number, style: TextStyle) => {
  switch (style) {
    case "background":
      return { x: fontSize * 0.4, y: fontSize * 0.2 };
    case "outline":
      return { x: fontSize * 0.14, y: fontSize * 0.08 };
    default:
      return { x: 0, y: 0 };
  }
};

/** Measures the box a text annotation occupies, in image pixels. */
export const measureTextBox = (
  text: string,
  fontSize: number,
  style: TextStyle,
) => {
  measureContext ??= new OffscreenCanvas(1, 1).getContext("2d");

  const lines = text.split("\n");
  const padding = textPadding(fontSize, style);
  let width = fontSize * 0.5;

  if (measureContext) {
    measureContext.font = textFont(fontSize);
    for (const line of lines) {
      width = Math.max(width, measureContext.measureText(line).width);
    }
  }

  return {
    height: Math.ceil(
      lines.length * fontSize * TEXT_LINE_HEIGHT + padding.y * 2,
    ),
    width: Math.ceil(width + padding.x * 2),
  };
};

/** Picks black or white, whichever reads better on `hex`. */
export const contrastColor = (hex: string) => {
  const value = Number.parseInt(hex.slice(1), 16);
  const red = (value >> 16) & 255;
  const green = (value >> 8) & 255;
  const blue = value & 255;
  const luminance = (0.299 * red + 0.587 * green + 0.114 * blue) / 255;

  return luminance > 0.62 ? "#000000" : "#FFFFFF";
};

export const drawText = (context: Canvas2D, shape: TextShape) => {
  const { color, fontSize, style, text } = shape;
  const padding = textPadding(fontSize, style);
  const lineHeight = fontSize * TEXT_LINE_HEIGHT;
  const lines = text.split("\n");

  context.save();
  context.font = textFont(fontSize);
  context.textBaseline = "middle";
  context.textAlign = "left";
  context.lineJoin = "round";

  if (style === "background") {
    context.fillStyle = color;
    context.beginPath();
    context.roundRect(
      shape.x,
      shape.y,
      shape.width,
      shape.height,
      fontSize * 0.25,
    );
    context.fill();
  }

  lines.forEach((line, index) => {
    const x = shape.x + padding.x;
    const y = shape.y + padding.y + lineHeight * (index + 0.5);

    if (style === "outline") {
      context.strokeStyle = contrastColor(color);
      context.lineWidth = fontSize * 0.18;
      context.strokeText(line, x, y);
    }

    context.fillStyle = style === "background" ? contrastColor(color) : color;
    context.fillText(line, x, y);
  });

  context.restore();
};
