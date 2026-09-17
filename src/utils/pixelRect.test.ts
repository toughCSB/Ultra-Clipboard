import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  hitRectHandle,
  intersectRects,
  moveRectInside,
  nudgeRect,
  rectFromEdges,
  resizeRectToEdge,
  snapEdgeInside,
} from "./pixelRect";

const BOUNDS = { height: 3840, width: 2160, x: 0, y: 0 };

describe("pixelRect", () => {
  it("snaps continuous points to pixel edges inside bounds", () => {
    assert.deepEqual(snapEdgeInside({ x: 99.6, y: -4 }, BOUNDS), {
      x: 100,
      y: 0,
    });
    assert.deepEqual(snapEdgeInside({ x: 5000, y: 3839.4 }, BOUNDS), {
      x: 2160,
      y: 3839,
    });
  });

  it("builds the same rectangle for any drag direction", () => {
    const forward = rectFromEdges({ x: 100, y: 100 }, { x: 420, y: 280 });
    const backward = rectFromEdges({ x: 420, y: 280 }, { x: 100, y: 100 });

    assert.deepEqual(forward, { height: 180, width: 320, x: 100, y: 100 });
    assert.deepEqual(backward, forward);
  });

  it("intersects rectangles and rejects touching edges", () => {
    assert.deepEqual(
      intersectRects(
        { height: 80, width: 100, x: 0, y: 0 },
        { height: 40, width: 100, x: 60, y: -10 },
      ),
      { height: 30, width: 40, x: 60, y: 0 },
    );
    assert.equal(
      intersectRects(
        { height: 10, width: 10, x: 0, y: 0 },
        { height: 10, width: 10, x: 10, y: 0 },
      ),
      null,
    );
  });

  it("keeps moved rectangles inside bounds", () => {
    const rect = { height: 100, width: 200, x: 2000, y: 10 };

    assert.deepEqual(moveRectInside(rect, 500, -50, BOUNDS), {
      height: 100,
      width: 200,
      x: 1960,
      y: 0,
    });
  });

  it("flips a resized rectangle when a corner crosses the opposite side", () => {
    const rect = { height: 100, width: 100, x: 100, y: 100 };

    assert.deepEqual(resizeRectToEdge(rect, "se", { x: 50, y: 60 }, BOUNDS), {
      height: 40,
      width: 50,
      x: 50,
      y: 60,
    });
    assert.deepEqual(resizeRectToEdge(rect, "e", { x: 150, y: 999 }, BOUNDS), {
      height: 100,
      width: 50,
      x: 100,
      y: 100,
    });
  });

  it("prefers corners over edges when hit testing handles", () => {
    const rect = { height: 100, width: 100, x: 100, y: 100 };

    assert.equal(hitRectHandle(rect, { x: 101, y: 99 }, 4), "nw");
    assert.equal(hitRectHandle(rect, { x: 150, y: 201 }, 4), "s");
    assert.equal(hitRectHandle(rect, { x: 150, y: 150 }, 4), null);
    assert.equal(hitRectHandle(rect, { x: 150, y: 90 }, 4), null);
  });

  it("nudges by one pixel and resizes with the modifier", () => {
    const rect = { height: 100, width: 100, x: 0, y: 0 };

    assert.deepEqual(nudgeRect(rect, "ArrowRight", 1, false, BOUNDS), {
      ...rect,
      x: 1,
    });
    assert.deepEqual(nudgeRect(rect, "ArrowLeft", 10, false, BOUNDS), rect);
    assert.deepEqual(nudgeRect(rect, "ArrowDown", 1, true, BOUNDS), {
      ...rect,
      height: 101,
    });
    assert.deepEqual(nudgeRect(rect, "ArrowLeft", 500, true, BOUNDS), {
      ...rect,
      width: 1,
    });
  });
});
