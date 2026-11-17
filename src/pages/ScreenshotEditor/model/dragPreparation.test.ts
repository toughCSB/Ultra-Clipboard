import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  createDragPreparationCache,
  type DragIdentity,
  sameDragIdentity,
} from "./dragPreparation";

const document = (id: string) => {
  return { id } as unknown as DragIdentity["document"];
};
const layer = (id: string) => {
  return { id } as unknown as NonNullable<DragIdentity["translation"]>;
};

const identity = (patch: Partial<DragIdentity> = {}): DragIdentity => {
  return {
    document: document("a"),
    selection: null,
    translation: null,
    ...patch,
  };
};

describe("screenshot drag identity", () => {
  it("is equal only when document, selection and translation are identical", () => {
    const base = identity();
    assert.equal(sameDragIdentity(base, { ...base }), true);
    assert.equal(sameDragIdentity(base, identity()), false);
    assert.equal(
      sameDragIdentity(base, {
        ...base,
        selection: { height: 1, width: 1, x: 0, y: 0 },
      }),
      false,
    );
    assert.equal(
      sameDragIdentity(base, { ...base, translation: layer("t") }),
      false,
    );
  });
});

describe("screenshot drag preparation cache", () => {
  it("reuses the prepared drag for the same identity", async () => {
    const cache = createDragPreparationCache();
    const wanted = identity();
    let renders = 0;
    const render = async () => {
      renders += 1;
      return true;
    };

    const first = cache.prepare(wanted, () => wanted, render);
    const second = cache.prepare({ ...wanted }, () => wanted, render);

    assert.equal(first, second);
    assert.equal(await first, true);
    assert.equal(renders, 1);
  });

  it("prepares again after the document, selection or translation changes", async () => {
    const cache = createDragPreparationCache();
    let live = identity();
    let renders = 0;
    const render = async () => {
      renders += 1;
      return true;
    };
    const run = async (wanted: DragIdentity) => {
      live = wanted;
      return await cache.prepare(wanted, () => live, render);
    };

    const base = identity();
    await run(base);
    await run(identity({ document: document("b") }));
    await run(
      identity({
        document: document("b"),
        selection: { height: 2, width: 2, x: 0, y: 0 },
      }),
    );
    await run(identity({ document: document("b"), translation: layer("t") }));

    assert.equal(renders, 4);
  });

  it("serialises renders: a newer preparation waits for the older one", async () => {
    const cache = createDragPreparationCache();
    const order: string[] = [];
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const a = identity();
    const b = identity({ document: document("b") });
    let live = a;

    const first = cache.prepare(
      a,
      () => live,
      async () => {
        order.push("a:start");
        await gate;
        order.push("a:end");
        return true;
      },
    );
    live = b;
    const second = cache.prepare(
      b,
      () => live,
      async () => {
        order.push("b:start");
        return true;
      },
    );
    release();

    assert.equal(await first, false);
    assert.equal(await second, true);
    assert.deepEqual(order, ["a:start", "a:end", "b:start"]);
  });

  it("skips rendering and reports false when the live identity moved on", async () => {
    const cache = createDragPreparationCache();
    let renders = 0;
    const ready = cache.prepare(
      identity(),
      () => identity({ document: document("other") }),
      async () => {
        renders += 1;
        return true;
      },
    );

    assert.equal(await ready, false);
    assert.equal(renders, 0);
  });

  it("reports false when the live identity changes during the render", async () => {
    const cache = createDragPreparationCache();
    const wanted = identity();
    let live = wanted;

    const ready = cache.prepare(
      wanted,
      () => live,
      async () => {
        live = identity({ document: document("later") });
        return true;
      },
    );

    assert.equal(await ready, false);
  });

  it("forgets a failed preparation, reports the error and retries next time", async () => {
    const cache = createDragPreparationCache();
    const wanted = identity();
    const errors: unknown[] = [];
    let renders = 0;

    const failed = cache.prepare(
      wanted,
      () => wanted,
      async () => {
        renders += 1;
        throw new Error("boom");
      },
      (error) => {
        errors.push(error);
      },
    );
    assert.equal(await failed, false);
    assert.equal(errors.length, 1);

    const retried = cache.prepare(
      wanted,
      () => wanted,
      async () => {
        renders += 1;
        return true;
      },
    );
    assert.equal(await retried, true);
    assert.equal(renders, 2);
  });

  it("invalidate drops the cached preparation", async () => {
    const cache = createDragPreparationCache();
    const wanted = identity();
    let renders = 0;
    const render = async () => {
      renders += 1;
      return true;
    };

    await cache.prepare(wanted, () => wanted, render);
    cache.invalidate();
    await cache.prepare(wanted, () => wanted, render);

    assert.equal(renders, 2);
  });
});
