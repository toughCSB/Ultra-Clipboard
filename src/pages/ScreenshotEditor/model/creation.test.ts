import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  createShape,
  dragCreatedShape,
  finalizeCreatedShape,
  grabsShape,
  measureRulers,
  placeMagnifier,
  refreshShape,
  type SourceImage,
} from "./creation";
import type {
  BlurShape,
  CounterShape,
  MagnifierShape,
  PenShape,
  RectShape,
  Shape,
} from "./shapes";
import { DEFAULT_TOOL_STYLE } from "./tools";

const AREA = { height: 1000, width: 1000, x: 0, y: 0 };
const INFO = { shiftKey: false, tolerance: 8 };
const IMAGE: SourceImage = {
  height: 4,
  pixels: new Uint8ClampedArray(4 * 4 * 4).fill(255),
  width: 4,
};

const rect = (overrides: Partial<RectShape> = {}): RectShape => {
  return {
    color: "#FF0000",
    fill: false,
    height: 50,
    id: "rect",
    kind: "rect",
    strokeWidth: 4,
    width: 100,
    x: 10,
    y: 20,
    ...overrides,
  };
};

const magnifier = (overrides: Partial<MagnifierShape> = {}): MagnifierShape => {
  return {
    height: 0,
    id: "lens",
    kind: "magnifier",
    sourceX: 100,
    sourceY: 100,
    strokeWidth: 3,
    width: 0,
    x: 100,
    y: 100,
    zoom: 2,
    ...overrides,
  };
};

describe("creation", () => {
  it("lets select tools grab any shape and the pen grab none", () => {
    const hit = rect();

    assert.equal(grabsShape(hit, "select", null), true);
    assert.equal(grabsShape(hit, "backdrop", null), true);
    assert.equal(grabsShape(hit, "pen", hit.id), false);
  });

  it("lets drawing tools grab the selected shape or their own kind", () => {
    const hit = rect();

    assert.equal(grabsShape(hit, "rect", null), true);
    assert.equal(grabsShape(hit, "arrow", hit.id), true);
    assert.equal(grabsShape(hit, "arrow", null), false);
  });

  it("starts shapes scaled to the capture", () => {
    const point = { x: 5, y: 6 };
    const style = { ...DEFAULT_TOOL_STYLE, strokeWidth: 4 };
    const line = createShape("line", point, style, 2, []);

    assert.equal(line?.kind, "line");
    assert.equal(line && "strokeWidth" in line ? line.strokeWidth : 0, 8);
    assert.equal(createShape("select", point, style, 2, []), null);
    assert.equal(createShape("text", point, style, 2, []), null);
  });

  it("numbers a new counter after the highest existing one", () => {
    const existing: CounterShape = {
      color: "#000000",
      id: "c",
      kind: "counter",
      radius: 10,
      value: 4,
      x: 0,
      y: 0,
    };
    const counter = createShape(
      "counter",
      { x: 1, y: 1 },
      DEFAULT_TOOL_STYLE,
      1,
      [existing],
    );

    assert.equal(counter?.kind === "counter" && counter.value, 5);
  });

  it("constrains a dragged line with shift and grows boxes from the origin", () => {
    const origin = { x: 0, y: 0 };
    const line = createShape(
      "line",
      origin,
      DEFAULT_TOOL_STYLE,
      1,
      [],
    ) as Shape;
    const dragged = dragCreatedShape(
      line,
      origin,
      { x: 100, y: 3 },
      { ...INFO, shiftKey: true },
    );

    assert.equal(dragged.kind === "line" && dragged.to.y, 0);

    const box = dragCreatedShape(
      rect({ height: 0, width: 0 }),
      origin,
      {
        x: 30,
        y: 40,
      },
      INFO,
    );

    assert.equal(box.kind === "rect" && box.width, 30);
    assert.equal(box.kind === "rect" && box.height, 40);
  });

  it("drops a box released as a click and keeps a dragged one", () => {
    assert.equal(
      finalizeCreatedShape(rect({ height: 2, width: 2 }), INFO, IMAGE, AREA, 1),
      null,
    );
    assert.deepEqual(
      finalizeCreatedShape(rect(), INFO, IMAGE, AREA, 1),
      rect(),
    );
  });

  it("snaps a blur and samples the erase color from its border", () => {
    const pixels = new Uint8ClampedArray(4 * 4 * 4);
    for (let index = 0; index < pixels.length; index += 4) {
      pixels.set([0, 0, 255, 255], index);
    }
    const blur: BlurShape = {
      fillColor: "#FFFFFF",
      height: 1.6,
      id: "blur",
      kind: "blur",
      mode: "erase",
      strength: 4,
      width: 2.4,
      x: 0.6,
      y: 0.4,
    };

    const erased = refreshShape(blur, { height: 4, pixels, width: 4 });

    assert.equal(erased.kind === "blur" && erased.fillColor, "#0000FF");
    assert.equal(erased.kind === "blur" && Number.isInteger(erased.x), true);

    const blurred = refreshShape({ ...blur, mode: "blur" }, IMAGE);

    assert.equal(blurred.kind === "blur" && blurred.fillColor, "#FFFFFF");
  });

  it("drops a blur smaller than two pixels on release", () => {
    const blur: BlurShape = {
      fillColor: "#FFFFFF",
      height: 1,
      id: "blur",
      kind: "blur",
      mode: "blur",
      strength: 4,
      width: 30,
      x: 0,
      y: 0,
    };

    assert.equal(finalizeCreatedShape(blur, INFO, IMAGE, AREA, 1), null);
  });

  it("adds pen points only after the pointer travels a quarter radius", () => {
    const pen: PenShape = {
      color: "#000000",
      id: "pen",
      kind: "pen",
      points: [{ x: 0, y: 0 }],
      strokeWidth: 2,
    };
    const origin = { x: 0, y: 0 };
    const near = dragCreatedShape(pen, origin, { x: 1, y: 0 }, INFO);
    const far = dragCreatedShape(pen, origin, { x: 3, y: 0 }, INFO);

    assert.equal(near, pen);
    assert.equal(far.kind === "pen" && far.points.length, 2);
  });

  it("places a clicked magnifier lens to the right of its source", () => {
    const lens = placeMagnifier(magnifier(), INFO, AREA, 1);

    assert.equal(lens.sourceX, 100);
    assert.equal(lens.width, 96);
    assert.equal(lens.x, 100 + 24 + 16);
  });

  it("flips the lens to the left when it would leave the area", () => {
    const lens = placeMagnifier(
      magnifier({ sourceX: 950, x: 950 }),
      INFO,
      AREA,
      1,
    );

    assert.equal(lens.x, 950 - 24 - 16 - 96);
  });

  it("measures a surface with one horizontal and one vertical ruler", () => {
    const [horizontal, vertical] = measureRulers(
      { bottom: 60, left: 10, right: 110, top: 20 },
      DEFAULT_TOOL_STYLE,
      1,
    );

    assert.deepEqual(horizontal.from, { x: 10, y: 40 });
    assert.deepEqual(horizontal.to, { x: 110, y: 40 });
    assert.deepEqual(vertical.from, { x: 60, y: 20 });
    assert.deepEqual(vertical.to, { x: 60, y: 60 });
    assert.notEqual(horizontal.id, vertical.id);
  });
});
