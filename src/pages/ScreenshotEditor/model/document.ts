import type { PixelRect } from "../../../utils/pixelRect";
import type { Shape } from "./shapes";

export type BackdropBackground =
  | "sunset"
  | "ocean"
  | "forest"
  | "graphite"
  | "light"
  | "transparent";

/** Padding, background, and frame drawn around the screenshot. */
export interface Backdrop {
  background: BackdropBackground;
  /** Padding on every side, in image pixels. */
  padding: number;
  /** Corner radius of the screenshot, in image pixels. */
  radius: number;
  shadow: boolean;
}

export interface EditorDocument {
  backdrop: Backdrop | null;
  /** Visible part of the captured image, in image pixels. */
  crop: PixelRect;
  /** Annotations in drawing order, in image pixels. */
  shapes: readonly Shape[];
}

export const BACKDROP_BACKGROUNDS: readonly BackdropBackground[] = [
  "sunset",
  "ocean",
  "forest",
  "graphite",
  "light",
  "transparent",
];

export const createDocument = (crop: PixelRect): EditorDocument => {
  return { backdrop: null, crop, shapes: [] };
};

/** Default backdrop sized relative to the screenshot, like a framed product shot. */
export const defaultBackdrop = (crop: PixelRect, scale: number): Backdrop => {
  const side = Math.min(crop.width, crop.height);

  return {
    background: "sunset",
    padding: Math.round(
      Math.max(24 * scale, Math.min(side * 0.08, 96 * scale)),
    ),
    radius: Math.round(10 * scale),
    shadow: true,
  };
};

/** Output area of the document: the crop plus any backdrop padding. */
export const documentBounds = (document: EditorDocument): PixelRect => {
  const padding = document.backdrop?.padding ?? 0;
  const { crop } = document;

  return {
    height: crop.height + padding * 2,
    width: crop.width + padding * 2,
    x: crop.x - padding,
    y: crop.y - padding,
  };
};

export const findShape = (document: EditorDocument, id: string | null) => {
  if (id === null) return null;

  return document.shapes.find((shape) => shape.id === id) ?? null;
};

export const addShape = (
  document: EditorDocument,
  shape: Shape,
): EditorDocument => {
  return { ...document, shapes: [...document.shapes, shape] };
};

export const addShapes = (
  document: EditorDocument,
  shapes: readonly Shape[],
): EditorDocument => {
  if (shapes.length === 0) return document;

  return { ...document, shapes: [...document.shapes, ...shapes] };
};

export const replaceShape = (
  document: EditorDocument,
  shape: Shape,
): EditorDocument => {
  return {
    ...document,
    shapes: document.shapes.map((current) => {
      return current.id === shape.id ? shape : current;
    }),
  };
};

export const removeShape = (
  document: EditorDocument,
  id: string,
): EditorDocument => {
  const shapes = document.shapes.filter((shape) => shape.id !== id);
  if (shapes.length === document.shapes.length) return document;

  return { ...document, shapes };
};

export const setBackdrop = (
  document: EditorDocument,
  backdrop: Backdrop | null,
): EditorDocument => {
  return { ...document, backdrop };
};
