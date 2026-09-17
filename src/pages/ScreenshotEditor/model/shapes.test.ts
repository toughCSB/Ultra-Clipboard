import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  boxFromPoints,
  constrainAngle,
  hitShape,
  hitShapeHandle,
  isDegenerateShape,
  type LineShape,
  moveShape,
  nextCounterValue,
  type RectShape,
  resizeShape,
  type Shape,
  shapeBounds,
  snapBox,
  topShapeAt,
} from "./shapes";

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

const arrow = (overrides: Partial<LineShape> = {}): LineShape => {
  return {
    color: "#FF0000",
    from: { x: 0, y: 0 },
    id: "arrow",
    kind: "arrow",
    strokeWidth: 4,
    to: { x: 100, y: 0 },
    ...overrides,
  };
};

describe("shapes", () => {
  it("builds boxes from drags in any direction and keeps squares", () => {
    assert.deepEqual(boxFromPoints({ x: 50, y: 40 }, { x: 10, y: 10 }), {
      height: 30,
      width: 40,
      x: 10,
      y: 10,
    });
    assert.deepEqual(boxFromPoints({ x: 50, y: 40 }, { x: 10, y: 30 }, true), {
      height: 40,
      width: 40,
      x: 10,
      y: 0,
    });
  });

  it("snaps boxes outward to whole pixels", () => {
    assert.deepEqual(snapBox({ height: 2.2, width: 3.1, x: 1.5, y: 0.4 }), {
      height: 3,
      width: 4,
      x: 1,
      y: 0,
    });
  });

  it("snaps segments to 45 degree steps", () => {
    const snapped = constrainAngle({ x: 0, y: 0 }, { x: 100, y: 10 });

    assert.equal(Math.round(snapped.y), 0);
    assert.equal(Math.round(snapped.x), 100);

    const diagonal = constrainAngle({ x: 0, y: 0 }, { x: 50, y: 45 });
    assert.ok(Math.abs(diagonal.x - diagonal.y) < 1e-6);
  });

  it("hits only the outline of an unfilled rectangle", () => {
    const shape = rect();

    assert.ok(hitShape(shape, { x: 10, y: 45 }, 2));
    assert.ok(!hitShape(shape, { x: 60, y: 45 }, 2));
    assert.ok(hitShape(rect({ fill: true }), { x: 60, y: 45 }, 2));
  });

  it("hits segments within the stroke and tolerance", () => {
    const shape = arrow();

    assert.ok(hitShape(shape, { x: 50, y: 3 }, 2));
    assert.ok(!hitShape(shape, { x: 50, y: 10 }, 2));
    assert.ok(!hitShape(shape, { x: 120, y: 0 }, 2));
  });

  it("finds the top-most shape", () => {
    const bottom = rect({ fill: true, id: "bottom" });
    const top = rect({ fill: true, id: "top" });

    assert.equal(topShapeAt([bottom, top], { x: 50, y: 40 }, 1)?.id, "top");
    assert.equal(topShapeAt([bottom, top], { x: 500, y: 40 }, 1), null);
  });

  it("moves every kind of geometry", () => {
    const moved = moveShape(arrow(), 5, -5) as LineShape;
    assert.deepEqual(moved.from, { x: 5, y: -5 });
    assert.deepEqual(moved.to, { x: 105, y: -5 });

    const pen: Shape = {
      color: "#000000",
      id: "pen",
      kind: "pen",
      points: [
        { x: 1, y: 1 },
        { x: 2, y: 3 },
      ],
      strokeWidth: 2,
    };
    assert.deepEqual((moveShape(pen, 1, 1) as typeof pen).points, [
      { x: 2, y: 2 },
      { x: 3, y: 4 },
    ]);

    assert.deepEqual(shapeBounds(moveShape(rect(), 1, 2)), {
      height: 50,
      width: 100,
      x: 11,
      y: 22,
    });
  });

  it("resizes boxes by handles and flips across the opposite edge", () => {
    const resized = resizeShape(rect(), "se", { x: 60, y: 120 }, false);
    assert.deepEqual(shapeBounds(resized), {
      height: 100,
      width: 50,
      x: 10,
      y: 20,
    });

    const flipped = resizeShape(rect(), "e", { x: 0, y: 0 }, false);
    assert.deepEqual(shapeBounds(flipped), {
      height: 50,
      width: 10,
      x: 0,
      y: 20,
    });
  });

  it("keeps the aspect ratio while resizing with a constraint", () => {
    const resized = resizeShape(rect(), "se", { x: 310, y: 30 }, true);
    const bounds = shapeBounds(resized);

    assert.equal(bounds.width / bounds.height, 2);
    assert.equal(bounds.width, 300);
    assert.equal(bounds.x, 10);
    assert.equal(bounds.y, 20);
  });

  it("moves segment endpoints by their handles", () => {
    const shape = arrow();
    const handle = hitShapeHandle(shape, { x: 99, y: 1 }, 4);

    assert.equal(handle, "end");
    const resized = resizeShape(shape, "end", { x: 40, y: 80 }, false);
    assert.deepEqual((resized as LineShape).to, { x: 40, y: 80 });
  });

  it("numbers counters after the largest existing value", () => {
    const counter = (value: number): Shape => {
      return {
        color: "#FF0000",
        id: `c${value}`,
        kind: "counter",
        radius: 10,
        value,
        x: 0,
        y: 0,
      };
    };

    assert.equal(nextCounterValue([]), 1);
    assert.equal(nextCounterValue([counter(1), rect(), counter(4)]), 5);
  });

  it("drops shapes that are too small after a drag", () => {
    assert.ok(isDegenerateShape(arrow({ to: { x: 1, y: 1 } }), 3));
    assert.ok(!isDegenerateShape(arrow(), 3));
    assert.ok(isDegenerateShape(rect({ width: 2 }), 3));
  });
});
