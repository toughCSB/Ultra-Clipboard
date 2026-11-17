import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { createDragGesture } from "./dragGesture";

function deferred() {
  let resolve!: (value: boolean) => void;
  const promise = new Promise<boolean>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("screenshot drag gesture", () => {
  it("does not start a native drag for a click without movement", async () => {
    const gesture = createDragGesture();
    let starts = 0;
    gesture.down(1, 10, 10);
    assert.equal(
      await gesture.move(
        { buttons: 1, pointerId: 1, x: 12, y: 12 },
        async () => {
          return true;
        },
        async () => {
          starts += 1;
          return;
        },
      ),
      false,
    );
    gesture.cancel();
    assert.equal(starts, 0);
  });

  it("pointer release or Escape during preparation prevents a late start", async () => {
    for (const cancel of ["pointerup", "Escape"]) {
      const gesture = createDragGesture();
      const prepared = deferred();
      let starts = 0;
      gesture.down(1, 0, 0);
      const pending = gesture.move(
        { buttons: 1, pointerId: 1, x: 8, y: 0 },
        () => prepared.promise,
        async () => {
          starts += 1;
          return;
        },
      );
      assert.equal(gesture.cancel(), true, cancel);
      prepared.resolve(true);
      assert.equal(await pending, false);
      assert.equal(starts, 0);
    }
  });

  it("starts once only while the same pointer remains held", async () => {
    const gesture = createDragGesture();
    const prepared = deferred();
    let starts = 0;
    gesture.down(4, 0, 0);
    const first = gesture.move(
      { buttons: 1, pointerId: 4, x: 7, y: 0 },
      () => prepared.promise,
      async () => {
        starts += 1;
        return;
      },
    );
    const duplicate = gesture.move(
      { buttons: 1, pointerId: 4, x: 9, y: 0 },
      () => prepared.promise,
      async () => {
        starts += 1;
        return;
      },
    );
    prepared.resolve(true);
    assert.equal(await first, true);
    assert.equal(await duplicate, false);
    assert.equal(starts, 1);
  });
});
