import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { ScreenshotTextRegion } from "@/commands";
import {
  textFromRegionRange,
  textRegionScreenRect,
  validTextRegions,
} from "./textSelection";

const regions: ScreenshotTextRegion[] = [
  { height: 18, lineIndex: 0, text: "Hello ", width: 42, x: 10, y: 20 },
  { height: 18, lineIndex: 0, text: "world", width: 48, x: 56, y: 20 },
  { height: 18, lineIndex: 1, text: "Again", width: 52, x: 10, y: 44 },
];

describe("screenshot text selection", () => {
  it("copies partial words in reading order with spaces and line breaks", () => {
    assert.equal(
      textFromRegionRange(
        regions,
        { index: 0, offset: 2 },
        { index: 2, offset: 3 },
      ),
      "llo world\nAga",
    );
    assert.equal(
      textFromRegionRange(
        regions,
        { index: 2, offset: 3 },
        { index: 0, offset: 2 },
      ),
      "llo world\nAga",
    );
    assert.equal(
      textFromRegionRange(
        regions,
        { index: 0, offset: 5 },
        { index: 0, offset: 6 },
      ),
      " ",
    );
  });

  it("places cropped and backdropped OCR boxes at the right zoom and HiDPI position", () => {
    assert.deepEqual(
      textRegionScreenRect(
        regions[0],
        { height: 120, width: 180, x: -20, y: 40 },
        { offsetX: 80, offsetY: -10, zoom: 1.5 },
        2,
      ),
      { height: 13.5, left: 32.5, top: 40, width: 31.5 },
    );
  });

  it("clips OCR boxes to the exported image and ignores boxes wholly outside", () => {
    assert.deepEqual(
      validTextRegions(
        [
          ...regions,
          { ...regions[0], text: "outside", x: 190 },
          { ...regions[0], text: "far away", x: 250 },
          { ...regions[0], width: -1 },
        ],
        { height: 100, width: 200, x: 0, y: 0 },
      ),
      [...regions, { ...regions[0], text: "outside", width: 10, x: 190 }],
    );
  });
});
