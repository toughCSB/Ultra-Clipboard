import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { averageBorderColor, measureSpans, readPixelHex } from "./pixels";

describe("pixels", () => {
  const pixels = new Uint8ClampedArray([
    0x19, 0x1a, 0x1d, 255, 255, 0, 8, 255, 1, 2, 3, 255, 250, 251, 252, 255,
  ]);

  it("reads the pixel that contains a fractional point", () => {
    assert.equal(readPixelHex(pixels, 2, 2, { x: 0.9, y: 0.2 }), "#191A1D");
    assert.equal(readPixelHex(pixels, 2, 2, { x: 1.5, y: 1.99 }), "#FAFBFC");
  });

  it("returns null outside the image", () => {
    assert.equal(readPixelHex(pixels, 2, 2, { x: -0.1, y: 0 }), null);
    assert.equal(readPixelHex(pixels, 2, 2, { x: 2, y: 0 }), null);
  });
});

/** 6×4 white image with a black 2×2 block at x 2..4, y 1..3. */
const blockImage = () => {
  const data = new Uint8ClampedArray(6 * 4 * 4).fill(255);

  for (let y = 1; y < 3; y += 1) {
    for (let x = 2; x < 4; x += 1) {
      const index = (y * 6 + x) * 4;
      data[index] = 0;
      data[index + 1] = 0;
      data[index + 2] = 0;
    }
  }

  return data;
};

describe("border color", () => {
  it("averages the ring around a box", () => {
    const data = blockImage();

    assert.equal(
      averageBorderColor(data, 6, 4, { height: 2, width: 2, x: 2, y: 1 }),
      "#FFFFFF",
    );
  });

  it("clips the ring at the image edges", () => {
    const data = blockImage();

    // The ring of a 1×1 box at (1, 0) keeps rows 0..1 and columns 0..2: one of six is black.
    assert.equal(
      averageBorderColor(data, 6, 4, { height: 1, width: 1, x: 1, y: 0 }),
      "#D5D5D5",
    );
  });
});

describe("measure spans", () => {
  const bounds = { height: 4, width: 6, x: 0, y: 0 };

  it("measures the same-colored run around a point", () => {
    const data = blockImage();

    assert.deepEqual(measureSpans(data, 6, 4, { x: 2.5, y: 1.5 }, bounds, 0), {
      bottom: 3,
      left: 2,
      right: 4,
      top: 1,
    });
  });

  it("stops at the bounds", () => {
    const data = blockImage();

    assert.deepEqual(measureSpans(data, 6, 4, { x: 0.5, y: 0.5 }, bounds, 0), {
      bottom: 4,
      left: 0,
      right: 6,
      top: 0,
    });
    assert.equal(measureSpans(data, 6, 4, { x: 7, y: 0 }, bounds, 0), null);
  });
});
