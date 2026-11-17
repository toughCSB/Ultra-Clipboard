import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { ScreenshotTranslationBlock } from "@/commands";
import {
  fitTranslationText,
  sampleTranslationBackground,
  wrapTranslationText,
} from "./translation";

const measure = (text: string, size: number) => {
  return Array.from(text).length * size;
};
const block: ScreenshotTranslationBlock = {
  height: 20,
  id: "block-0",
  sourceText: "Welcome",
  translatedText: "안녕하세요 반갑습니다",
  width: 80,
  x: 5,
  y: 5,
};

describe("translation image layout", () => {
  it("wraps Korean and Unicode code points within the available width", () => {
    const text = "이미지 번역 테스트 😊";
    const lines = wrapTranslationText(text, 40, 10, measure);
    assert.ok(
      lines.every((line) => {
        return measure(line, 10) <= 40;
      }),
    );
    assert.equal(lines.join("").replace(/\s/g, ""), text.replace(/\s/g, ""));
    assert.ok(
      lines.some((line) => {
        return line.includes("😊");
      }),
    );
  });
  it("uses free vertical space before reducing readable text size", () => {
    const layout = fitTranslationText(block, 100, measure);
    assert.equal(layout.fontSize, block.height * 0.78);
    assert.equal(layout.overflow, false);
    assert.ok(layout.height > block.height && layout.height <= 100);
  });
  it("marks overflowing translations instead of silently losing them", () => {
    const input = {
      ...block,
      translatedText: "아주 긴 번역 문장입니다 ".repeat(20),
    };
    const layout = fitTranslationText(input, block.height, measure);
    assert.equal(layout.overflow, true);
    assert.ok(layout.lines[layout.lines.length - 1]?.endsWith("…"));
    assert.equal(input.translatedText, "아주 긴 번역 문장입니다 ".repeat(20));
    assert.ok(layout.height <= block.height);
  });
  it("samples source background and chooses contrasting text", () => {
    const pixels = new Uint8Array(8 * 8 * 4);
    for (let index = 0; index < pixels.length; index += 4)
      pixels.set([20, 20, 20, 255], index);
    const sampled = sampleTranslationBackground(pixels, 8, 8, {
      height: 5,
      width: 5,
      x: 1,
      y: 1,
    });
    assert.equal(sampled.background, "rgb(20, 20, 20)");
    assert.equal(sampled.foreground, "#F5F5F5");
  });
  const sampleInk = (
    background: readonly number[],
    colors: readonly (readonly number[])[],
    textured = false,
  ) => {
    const width = 100;
    const height = 40;
    const pixels = new Uint8Array(width * height * 4);
    for (let y = 0; y < height; y += 1) {
      for (let x = 0; x < width; x += 1) {
        const base = textured && x % 4 < 2 ? [30, 90, 160] : background;
        pixels.set([...base, 255], (y * width + x) * 4);
        if (y >= 14 && y < 25 && x >= 15 && x < 85 && x % 5 < 2) {
          const color = colors[Math.floor(x / 5) % colors.length];
          const ink =
            y === 14
              ? color.map((value, channel) => {
                  return Math.round((value + base[channel]) / 2);
                })
              : color;
          pixels.set([...ink, 255], (y * width + x) * 4);
        }
      }
    }
    return sampleTranslationBackground(pixels, width, height, {
      height: 20,
      width: 80,
      x: 10,
      y: 10,
    });
  };
  it("preserves colored ink on light and dark backgrounds despite antialiased edges", () => {
    for (const color of [
      [200, 32, 48],
      [24, 96, 216],
      [20, 20, 20],
    ]) {
      assert.equal(
        sampleInk([255, 255, 255], [color]).foreground,
        `rgb(${color.join(", ")})`,
      );
    }
    assert.equal(
      sampleInk([20, 20, 20], [[240, 200, 40]]).foreground,
      "rgb(240, 200, 40)",
    );
    assert.equal(
      sampleInk([20, 20, 20], [[255, 255, 255]]).foreground,
      "rgb(255, 255, 255)",
    );
  });
  it("falls back when multiple ink colors have no reliable dominant color", () => {
    assert.equal(
      sampleInk(
        [255, 255, 255],
        [
          [200, 32, 48],
          [24, 96, 216],
        ],
      ).foreground,
      "#111111",
    );
  });
  it("falls back on textured backgrounds and low contrast source pixels", () => {
    assert.equal(
      sampleInk([255, 255, 255], [[200, 32, 48]], true).foreground,
      "#F5F5F5",
    );
    assert.equal(
      sampleInk([255, 255, 255], [[240, 240, 240]]).foreground,
      "#111111",
    );
  });
});
