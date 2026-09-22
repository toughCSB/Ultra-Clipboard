import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { applyStyleToShape, styleFromShape } from "./shapeStyle";
import type {
  BlurShape,
  HighlighterShape,
  RectShape,
  TextShape,
} from "./shapes";
import { DEFAULT_TOOL_STYLE } from "./tools";

const IMAGE = {
  height: 4,
  pixels: new Uint8ClampedArray(4 * 4 * 4).fill(200),
  width: 4,
};

const measure = (text: string, fontSize: number) => {
  return { height: fontSize, width: text.length * fontSize };
};

describe("shape style", () => {
  it("converts widths between logical and image pixels", () => {
    const shape: RectShape = {
      color: "#FF0000",
      fill: false,
      height: 10,
      id: "r",
      kind: "rect",
      strokeWidth: 5,
      width: 10,
      x: 0,
      y: 0,
    };

    assert.equal(
      styleFromShape(DEFAULT_TOOL_STYLE, shape, 1.25).strokeWidth,
      4,
    );

    const next = applyStyleToShape(
      shape,
      { color: "#0000FF", strokeWidth: 8 },
      1.25,
      IMAGE,
      measure,
    ) as RectShape;

    assert.equal(next.strokeWidth, 10);
    assert.equal(next.color, "#0000FF");
    assert.equal(next.fill, false);
  });

  it("re-measures text when its size changes", () => {
    const shape: TextShape = {
      color: "#000000",
      fontSize: 20,
      height: 20,
      id: "t",
      kind: "text",
      style: "plain",
      text: "abc",
      width: 60,
      x: 0,
      y: 0,
    };
    const next = applyStyleToShape(
      shape,
      { textSize: 16 },
      2,
      IMAGE,
      measure,
    ) as TextShape;

    assert.equal(next.fontSize, 32);
    assert.equal(next.width, 96);
  });

  it("keeps each highlight's color and intensity editable", () => {
    const shape: HighlighterShape = {
      color: "#FFE600",
      height: 12,
      id: "h",
      kind: "highlighter",
      opacity: 75,
      width: 60,
      x: 2,
      y: 3,
    };
    const next = applyStyleToShape(
      shape,
      { highlightColor: "#FF9BD2", highlightOpacity: 90 },
      2,
      IMAGE,
      measure,
    ) as HighlighterShape;

    assert.equal(next.color, "#FF9BD2");
    assert.equal(next.opacity, 90);
    assert.equal(next.width, shape.width);
    assert.equal(
      styleFromShape(DEFAULT_TOOL_STYLE, next, 2).highlightOpacity,
      90,
    );
    assert.equal(shape.opacity, 75);
  });

  it("samples a fill color when a blur switches to erase", () => {
    const shape: BlurShape = {
      fillColor: "#FFFFFF",
      height: 2,
      id: "b",
      kind: "blur",
      mode: "blur",
      strength: 12,
      width: 2,
      x: 1,
      y: 1,
    };
    const next = applyStyleToShape(
      shape,
      { blurMode: "erase" },
      1,
      IMAGE,
      measure,
    ) as BlurShape;

    assert.equal(next.mode, "erase");
    assert.equal(next.fillColor, "#C8C8C8");
  });
});
