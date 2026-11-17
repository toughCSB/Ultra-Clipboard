import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { HighlighterShape } from "../model/shapes";
import type { Canvas2D } from "./drawDocument";
import { drawShape } from "./drawShapes";

describe("highlighter rendering", () => {
  it("renders one straight marker segment twice with its chosen thickness and opacity", () => {
    const calls: string[] = [];
    const strokes: { alpha: number; composite: string; width: number }[] = [];
    const context = {
      beginPath() {
        calls.push("begin");
      },
      globalAlpha: 1,
      globalCompositeOperation: "source-over",
      lineTo() {
        calls.push("line");
      },
      lineWidth: 0,
      moveTo() {
        calls.push("move");
      },
      quadraticCurveTo() {
        calls.push("curve");
      },
      restore() {
        calls.push("restore");
      },
      roundRect() {
        throw new Error("marker must not draw a rectangle");
      },
      save() {
        calls.push("save");
      },
      stroke() {
        strokes.push({
          alpha: context.globalAlpha,
          composite: context.globalCompositeOperation,
          width: context.lineWidth,
        });
      },
    };
    const shape: HighlighterShape = {
      color: "#FFE600",
      from: { x: 10, y: 10 },
      id: "marker",
      kind: "highlighter",
      opacity: 60,
      strokeWidth: 24,
      to: { x: 40, y: 50 },
    };

    drawShape(context as unknown as Canvas2D, {} as ImageBitmap, shape);

    assert.equal(calls.filter((call) => call === "curve").length, 0);
    assert.equal(calls.filter((call) => call === "line").length, 2);
    assert.deepEqual(strokes, [
      { alpha: 0.15, composite: "source-over", width: 24 },
      { alpha: 0.6, composite: "multiply", width: 24 },
    ]);
  });
});
