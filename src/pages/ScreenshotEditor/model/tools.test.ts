import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  ALL_TOOLS,
  DEFAULT_TOOL_ORDER,
  DEFAULT_TOOL_STYLE,
  parseToolOrder,
  parseToolStyle,
  STROKE_RANGE,
  toolByCode,
} from "./tools";

describe("tools", () => {
  it("gives every tool a unique single-key shortcut", () => {
    const codes = ALL_TOOLS.map((tool) => tool.code);

    assert.equal(new Set(codes).size, codes.length);
    assert.equal(toolByCode("KeyA")?.id, "arrow");
    assert.equal(toolByCode("KeyZ"), null);
  });

  it("falls back field by field for stored options", () => {
    const parsed = parseToolStyle({
      blurMode: "smudge",
      color: "#00ff00",
      strokeWidth: 999,
      textStyle: "background",
    });

    assert.equal(parsed.blurMode, DEFAULT_TOOL_STYLE.blurMode);
    assert.equal(parsed.color, "#00ff00");
    assert.equal(parsed.strokeWidth, STROKE_RANGE.max);
    assert.equal(parsed.textStyle, "background");
    assert.deepEqual(parseToolStyle(null), DEFAULT_TOOL_STYLE);
  });

  it("gives every tool a distinct icon color", () => {
    const colors = ALL_TOOLS.map((tool) => tool.color);

    assert.equal(new Set(colors).size, colors.length);
  });

  it("keeps a stored tool order but drops unknown ids and appends new ones", () => {
    const stored = ["backdrop", "select", "not-a-real-tool", "arrow"];
    const parsed = parseToolOrder(stored);

    assert.deepEqual(parsed.slice(0, 3), ["backdrop", "select", "arrow"]);
    assert.equal(parsed.length, DEFAULT_TOOL_ORDER.length);
    assert.deepEqual([...parsed].sort(), [...DEFAULT_TOOL_ORDER].sort());
  });

  it("falls back to the default order for garbage input", () => {
    assert.deepEqual(parseToolOrder(null), DEFAULT_TOOL_ORDER);
    assert.deepEqual(parseToolOrder("nonsense"), DEFAULT_TOOL_ORDER);
  });
});
