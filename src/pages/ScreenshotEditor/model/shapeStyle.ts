import { averageBorderColor } from "./pixels";
import { type Shape, snapBox } from "./shapes";
import type { ToolStyle } from "./tools";

export interface StyleImage {
  height: number;
  pixels: Uint8ClampedArray;
  width: number;
}

/** Reads the options panel values from a selected shape, in logical pixels. */
export const styleFromShape = (
  base: ToolStyle,
  shape: Shape,
  scale: number,
): ToolStyle => {
  switch (shape.kind) {
    case "arrow":
    case "line":
    case "pen":
      return {
        ...base,
        color: shape.color,
        strokeWidth: shape.strokeWidth / scale,
      };
    case "rect":
    case "oval":
      return {
        ...base,
        color: shape.color,
        rectFill: shape.fill,
        strokeWidth: shape.strokeWidth / scale,
      };
    case "ruler":
      return {
        ...base,
        color: shape.color,
        strokeWidth: (shape.strokeWidth / scale) * 2,
      };
    case "highlighter":
      return { ...base, highlightColor: shape.color };
    case "blur":
      return {
        ...base,
        blurMode: shape.mode,
        strokeWidth: shape.strength / scale / 3,
      };
    case "spotlight":
      return { ...base, spotlightOval: shape.oval };
    case "counter":
      return {
        ...base,
        color: shape.color,
        counterSize: shape.radius / scale,
      };
    case "magnifier":
      return { ...base, magnifierZoom: shape.zoom };
    case "text":
      return {
        ...base,
        color: shape.color,
        textSize: shape.fontSize / scale,
        textStyle: shape.style,
      };
    case "image":
      return base;
  }
};

/**
 * Applies changed options to a selected shape. Only fields that the shape uses
 * change; `measureText` re-measures text whose font changed.
 */
export const applyStyleToShape = (
  shape: Shape,
  patch: Partial<ToolStyle>,
  scale: number,
  image: StyleImage,
  measureText: (
    text: string,
    fontSize: number,
    style: ToolStyle["textStyle"],
  ) => { height: number; width: number },
): Shape => {
  switch (shape.kind) {
    case "arrow":
    case "line":
    case "pen":
      return {
        ...shape,
        color: patch.color ?? shape.color,
        strokeWidth:
          patch.strokeWidth === undefined
            ? shape.strokeWidth
            : patch.strokeWidth * scale,
      };
    case "rect":
    case "oval":
      return {
        ...shape,
        color: patch.color ?? shape.color,
        fill: patch.rectFill ?? shape.fill,
        strokeWidth:
          patch.strokeWidth === undefined
            ? shape.strokeWidth
            : patch.strokeWidth * scale,
      };
    case "ruler":
      return {
        ...shape,
        color: patch.color ?? shape.color,
        fontSize:
          patch.strokeWidth === undefined
            ? shape.fontSize
            : Math.max(11, patch.strokeWidth * 3 + 8) * scale,
        strokeWidth:
          patch.strokeWidth === undefined
            ? shape.strokeWidth
            : Math.max(1, patch.strokeWidth / 2) * scale,
      };
    case "highlighter":
      return { ...shape, color: patch.highlightColor ?? shape.color };
    case "blur": {
      const mode = patch.blurMode ?? shape.mode;
      const box = snapBox(shape);

      return {
        ...shape,
        fillColor:
          mode === "erase"
            ? averageBorderColor(image.pixels, image.width, image.height, box)
            : shape.fillColor,
        mode,
        strength:
          patch.strokeWidth === undefined
            ? shape.strength
            : Math.max(4, patch.strokeWidth * 3) * scale,
      };
    }
    case "spotlight":
      return { ...shape, oval: patch.spotlightOval ?? shape.oval };
    case "counter":
      return {
        ...shape,
        color: patch.color ?? shape.color,
        radius:
          patch.counterSize === undefined
            ? shape.radius
            : patch.counterSize * scale,
      };
    case "magnifier": {
      if (patch.magnifierZoom === undefined) return shape;

      return { ...shape, zoom: patch.magnifierZoom };
    }
    case "text": {
      const fontSize =
        patch.textSize === undefined ? shape.fontSize : patch.textSize * scale;
      const style = patch.textStyle ?? shape.style;
      const size = measureText(shape.text, fontSize, style);

      return {
        ...shape,
        ...size,
        color: patch.color ?? shape.color,
        fontSize,
        style,
      };
    }
    case "image":
      return shape;
  }
};
