import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  commitHistory,
  createHistory,
  redoHistory,
  undoHistory,
} from "./history";

describe("history", () => {
  it("undoes and redoes committed documents in order", () => {
    const first = { crop: 1 };
    const second = { crop: 2 };
    const third = { crop: 3 };
    let history = createHistory(first);

    history = commitHistory(history, second);
    history = commitHistory(history, third);
    history = undoHistory(history);

    assert.equal(history.present, second);

    history = undoHistory(history);

    assert.equal(history.present, first);
    assert.equal(undoHistory(history), history);

    history = redoHistory(redoHistory(history));

    assert.equal(history.present, third);
    assert.equal(redoHistory(history), history);
  });

  it("ignores commits of the current document", () => {
    const document = { crop: 1 };
    const history = createHistory(document);

    assert.equal(commitHistory(history, document), history);
  });

  it("drops redo steps after a new commit", () => {
    let history = createHistory("a");

    history = commitHistory(history, "b");
    history = undoHistory(history);
    history = commitHistory(history, "c");

    assert.deepEqual(history.future, []);
    assert.deepEqual(history.past, ["a"]);
  });

  it("keeps only the newest steps within the limit", () => {
    let history = createHistory(0);

    for (let step = 1; step <= 5; step += 1) {
      history = commitHistory(history, step, 3);
    }

    assert.deepEqual(history.past, [2, 3, 4]);
    assert.equal(history.present, 5);
  });
});
