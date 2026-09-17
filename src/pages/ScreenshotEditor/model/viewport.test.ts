import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  clampViewport,
  fitViewport,
  MAX_ZOOM,
  toImagePoint,
  zoomViewportAt,
} from "./viewport";

const PORTRAIT = { height: 3840, width: 2160, x: 0, y: 0 };

describe("viewport", () => {
  it("fits a large capture inside the view", () => {
    const viewport = fitViewport(PORTRAIT, 1944, 3000, 48);

    assert.equal(viewport.zoom, (3000 - 96) / 3840);
    assert.ok(viewport.offsetY >= 47 && viewport.offsetY <= 49);
  });

  it("never enlarges small captures past actual size", () => {
    const viewport = fitViewport(
      { height: 180, width: 320, x: 0, y: 0 },
      1200,
      700,
      48,
    );

    assert.deepEqual(viewport, { offsetX: 440, offsetY: 260, zoom: 1 });
  });

  it("centers cropped content by its own origin", () => {
    const viewport = fitViewport(
      { height: 100, width: 200, x: 500, y: 300 },
      400,
      300,
      0,
    );

    assert.deepEqual(toImagePoint(viewport, 200, 150), { x: 600, y: 350 });
  });

  it("keeps the anchor point fixed while zooming", () => {
    const viewport = { offsetX: 10, offsetY: 20, zoom: 0.5 };
    const before = toImagePoint(viewport, 300, 200);
    const zoomed = zoomViewportAt(viewport, 4, 300, 200);

    assert.deepEqual(toImagePoint(zoomed, 300, 200), before);
    assert.equal(zoomViewportAt(viewport, 1000, 0, 0).zoom, MAX_ZOOM);
  });

  it("keeps part of the content visible after panning far away", () => {
    const clamped = clampViewport(
      { offsetX: -10_000, offsetY: 9_000, zoom: 1 },
      { height: 180, width: 320, x: 0, y: 0 },
      800,
      600,
      64,
    );

    assert.deepEqual(clamped, { offsetX: -256, offsetY: 536, zoom: 1 });
  });
});
