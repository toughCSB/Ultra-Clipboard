import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { ScreenshotTranslationBlock } from "@/commands";
import type { PixelRect } from "@/utils/pixelRect";
import type { TranslationLayer } from "../render/translation";
import { createDocument, type EditorDocument } from "./document";
import {
  activeLayer,
  createTranslationRequestIds,
  failTranslation,
  finishTranslation,
  isAwaitingKey,
  isLoading,
  isTranslationStale,
  requireKey,
  resolveTranslation,
  settleCancelled,
  startKeyCheck,
  startTranslating,
  toggleOriginal,
  translatedText,
} from "./translationSession";

const area: PixelRect = { height: 20, width: 40, x: 0, y: 0 };
const otherSelection: PixelRect = { height: 5, width: 5, x: 1, y: 1 };
const document: EditorDocument = createDocument(area);
const otherDocument: EditorDocument = createDocument(area);
const layer: TranslationLayer = { area, blocks: [], overflowCount: 0 };

const block = (id: string, translatedText: string) => {
  return {
    height: 1,
    id,
    sourceText: "src",
    translatedText,
    width: 1,
    x: 0,
    y: 0,
  } satisfies ScreenshotTranslationBlock;
};

describe("translation request ids", () => {
  it("are positive safe integers that only grow", () => {
    const ids = createTranslationRequestIds(() => 1_000);
    const first = ids.next();
    const second = ids.next();
    const third = ids.next();

    assert.equal(first, 1_000_000);
    assert.equal(second, first + 1);
    assert.equal(third, second + 1);
  });

  it("keep growing when the clock moves backwards", () => {
    let time = 2_000;
    const ids = createTranslationRequestIds(() => time);
    const first = ids.next();
    time = 1_000;

    assert.ok(ids.next() > first);
  });

  it("refuse values outside the safe integer range", () => {
    assert.throws(() => createTranslationRequestIds(() => 2 ** 53).next());
    assert.throws(() => createTranslationRequestIds(() => Number.NaN).next());
  });
});

describe("translation staleness", () => {
  const token = { document, generation: 3, selection: null };
  const current = { ...token, closing: false };

  it("accepts a result that still matches the editor", () => {
    assert.equal(isTranslationStale(token, current), false);
  });

  it("ignores an old request", () => {
    assert.equal(
      isTranslationStale(token, { ...current, generation: 4 }),
      true,
    );
  });

  it("ignores a changed document", () => {
    assert.equal(
      isTranslationStale(token, { ...current, document: otherDocument }),
      true,
    );
  });

  it("ignores a changed selection", () => {
    assert.equal(
      isTranslationStale(token, { ...current, selection: otherSelection }),
      true,
    );
  });

  it("ignores everything while the window is closing", () => {
    assert.equal(
      isTranslationStale(token, { ...current, closing: true }),
      true,
    );
  });
});

describe("translation session transitions", () => {
  it("walks checking the key, needing one, translating and ready", () => {
    const checking = startKeyCheck(document, null, area);
    assert.equal(checking.phase, "checkingKey");
    assert.equal(isLoading(checking), true);
    assert.equal(isAwaitingKey(checking), true);

    const needs = requireKey(checking);
    assert.equal(needs.phase, "needsKey");
    assert.equal(isLoading(needs), true);
    assert.equal(isAwaitingKey(needs), true);

    const translating = startTranslating(document, null, area);
    assert.equal(translating.phase, "translating");
    assert.equal(isAwaitingKey(translating), false);
    assert.equal(activeLayer(translating), null);

    const ready = finishTranslation(translating, [block("a", "가")], layer);
    assert.equal(ready.phase, "ready");
    assert.equal(isLoading(ready), false);
    assert.equal(activeLayer(ready), layer);
  });

  it("keeps the failed attempt so it can be retried", () => {
    const failed = failTranslation(startTranslating(document, null, area));

    assert.equal(failed.phase, "failed");
    assert.equal(isLoading(failed), false);
    assert.equal(failed.document, document);
  });

  it("shows the original without dropping the layer and toggles back", () => {
    const ready = finishTranslation(
      startTranslating(document, null, area),
      [block("a", "가")],
      layer,
    );
    const original = toggleOriginal(ready);

    assert.equal(original.showOriginal, true);
    assert.equal(original.layer, layer);
    assert.equal(activeLayer(original), null);
    assert.equal(toggleOriginal(original).showOriginal, false);
  });

  it("does not toggle while loading", () => {
    const loading = startTranslating(document, null, area);

    assert.equal(toggleOriginal(loading), loading);
    assert.equal(activeLayer(null), null);
  });

  it("joins only non-empty translated blocks", () => {
    const ready = finishTranslation(
      startTranslating(document, null, area),
      [block("a", "가"), block("b", "  "), block("c", "나")],
      layer,
    );

    assert.equal(translatedText(ready), "가\n나");
  });
});

describe("translation result", () => {
  it("treats a cancelled export as cancelled even without blocks", () => {
    assert.deepEqual(
      resolveTranslation({
        translationBlocks: null,
        translationCancelled: true,
      }),
      { kind: "cancelled" },
    );
  });

  it("keeps empty blocks as an empty result for the no-text message", () => {
    assert.deepEqual(
      resolveTranslation({
        translationBlocks: [],
        translationCancelled: false,
      }),
      { blocks: [], kind: "blocks" },
    );
    assert.deepEqual(
      resolveTranslation({
        translationBlocks: null,
        translationCancelled: false,
      }),
      { blocks: [], kind: "blocks" },
    );
  });

  it("returns the translated blocks", () => {
    const blocks = [block("a", "가")];

    assert.deepEqual(
      resolveTranslation({
        translationBlocks: blocks,
        translationCancelled: false,
      }),
      { blocks, kind: "blocks" },
    );
  });

  it("settles a cancel back to a failed attempt or to idle", () => {
    const failed = failTranslation(startTranslating(document, null, area));

    assert.equal(settleCancelled(failed), failed);
    assert.equal(
      settleCancelled(requireKey(startKeyCheck(document, null, area))),
      null,
    );
    assert.equal(settleCancelled(null), null);
  });
});
