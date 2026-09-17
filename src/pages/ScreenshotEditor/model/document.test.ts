import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  addShape,
  createDocument,
  defaultBackdrop,
  documentBounds,
  findShape,
  removeShape,
  replaceShape,
  setBackdrop,
} from "./document";
import type { Shape } from "./shapes";

const CROP = { height: 200, width: 300, x: 40, y: 60 };

const counter = (id: string, value: number): Shape => {
  return {
    color: "#FF0000",
    id,
    kind: "counter",
    radius: 10,
    value,
    x: 0,
    y: 0,
  };
};

describe("document", () => {
  it("uses the crop as bounds without a backdrop", () => {
    assert.deepEqual(documentBounds(createDocument(CROP)), CROP);
  });

  it("grows the bounds by the backdrop padding", () => {
    const document = setBackdrop(createDocument(CROP), {
      background: "ocean",
      padding: 24,
      radius: 8,
      shadow: true,
    });

    assert.deepEqual(documentBounds(document), {
      height: 248,
      width: 348,
      x: 16,
      y: 36,
    });
  });

  it("sizes the default backdrop from the capture scale", () => {
    const backdrop = defaultBackdrop(
      { height: 3840, width: 2160, x: 0, y: 0 },
      1.25,
    );

    assert.equal(backdrop.padding, 120);
    assert.equal(backdrop.radius, 13);
  });

  it("adds, replaces, and removes shapes without mutating earlier documents", () => {
    const empty = createDocument(CROP);
    const withOne = addShape(empty, counter("a", 1));
    const replaced = replaceShape(withOne, counter("a", 7));
    const removed = removeShape(replaced, "a");

    assert.equal(empty.shapes.length, 0);
    assert.equal(findShape(withOne, "a")?.kind, "counter");
    assert.equal((findShape(replaced, "a") as { value: number }).value, 7);
    assert.equal((findShape(withOne, "a") as { value: number }).value, 1);
    assert.equal(removed.shapes.length, 0);
    assert.equal(removeShape(removed, "missing"), removed);
  });
});
