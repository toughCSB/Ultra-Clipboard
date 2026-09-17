import type { PixelRect } from "@/utils/pixelRect";
import { drawDocument, type EditorScene } from "./drawDocument";

/** Renders `area` of the scene at actual size and returns straight RGBA bytes. */
export const renderScenePixels = (scene: EditorScene, area: PixelRect) => {
  const canvas = new OffscreenCanvas(area.width, area.height);
  const context = canvas.getContext("2d", { willReadFrequently: true });

  if (!context) {
    throw new Error("2d canvas context is unavailable");
  }

  context.imageSmoothingEnabled = false;
  context.setTransform(1, 0, 0, 1, -area.x, -area.y);
  drawDocument(context, scene, area);

  const { data } = context.getImageData(0, 0, area.width, area.height);

  return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
};
