import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  createOverlaySession,
  isCurrentLoad,
  type OverlaySession,
  type OverlaySessionCommand,
  type OverlaySnapshot,
  overlaySessionReducer,
} from "./overlaySession";

const RECT = { height: 20, width: 30, x: 5, y: 6 };

function snapshot(patch: Partial<OverlaySnapshot> = {}): OverlaySnapshot {
  return { mode: "area", ready: true, sessionId: 1, ...patch };
}

function load(
  session: OverlaySession,
  snap: OverlaySnapshot | null,
): { commands: OverlaySessionCommand[]; session: OverlaySession } {
  const started = overlaySessionReducer(session, { type: "loadStarted" });

  return overlaySessionReducer(started.session, {
    generation: started.session.generation,
    snapshot: snap,
    type: "stateLoaded",
  });
}

describe("overlaySession", () => {
  it("queues a commit made before pixels arrive and sends it once on ready", () => {
    const early = load(createOverlaySession(), snapshot({ ready: false }));

    const requested = overlaySessionReducer(early.session, {
      rect: RECT,
      type: "commitRequested",
    });
    assert.deepEqual(requested.commands, []);

    const again = overlaySessionReducer(requested.session, {
      rect: RECT,
      type: "commitRequested",
    });
    assert.deepEqual(again.commands, []);

    const ready = load(again.session, snapshot({ ready: true }));
    assert.deepEqual(ready.commands, [
      { rect: RECT, sessionId: 1, type: "sendCommit" },
    ]);

    const reloaded = load(ready.session, snapshot({ ready: true }));
    assert.deepEqual(reloaded.commands, []);
  });

  it("sends a commit immediately when ready and ignores a second one", () => {
    const { session } = load(createOverlaySession(), snapshot());

    const first = overlaySessionReducer(session, {
      rect: RECT,
      type: "commitRequested",
    });
    assert.deepEqual(first.commands, [
      { rect: RECT, sessionId: 1, type: "sendCommit" },
    ]);

    const second = overlaySessionReducer(first.session, {
      rect: RECT,
      type: "commitRequested",
    });
    assert.deepEqual(second.commands, []);

    const failed = overlaySessionReducer(first.session, {
      type: "commitFailed",
    });
    const retry = overlaySessionReducer(failed.session, {
      rect: RECT,
      type: "commitRequested",
    });
    assert.equal(retry.commands.length, 1);
  });

  it("ignores a commit request without a session", () => {
    const result = overlaySessionReducer(createOverlaySession(), {
      rect: RECT,
      type: "commitRequested",
    });

    assert.deepEqual(result.commands, []);
  });

  it("ignores a frame from an old load generation", () => {
    const first = overlaySessionReducer(createOverlaySession(), {
      type: "loadStarted",
    });
    const second = overlaySessionReducer(first.session, {
      type: "loadStarted",
    });

    assert.equal(
      isCurrentLoad(second.session, first.session.generation),
      false,
    );
    assert.equal(
      isCurrentLoad(second.session, second.session.generation),
      true,
    );

    const stale = overlaySessionReducer(second.session, {
      generation: first.session.generation,
      snapshot: snapshot({ sessionId: 9 }),
      type: "stateLoaded",
    });
    assert.deepEqual(stale.commands, []);
    assert.equal(stale.session, second.session);
  });

  it("keeps the selection on a same-session reload and clears it for a new session", () => {
    const first = load(createOverlaySession(), snapshot({ ready: false }));
    assert.deepEqual(first.commands, [{ mode: "area", type: "startSession" }]);

    const same = load(first.session, snapshot({ ready: true }));
    assert.deepEqual(same.commands, []);

    const next = load(same.session, snapshot({ mode: "window", sessionId: 2 }));
    assert.deepEqual(next.commands, [{ mode: "window", type: "startSession" }]);
  });

  it("drops a pending commit when a new session starts", () => {
    const first = load(createOverlaySession(), snapshot({ ready: false }));
    const queued = overlaySessionReducer(first.session, {
      rect: RECT,
      type: "commitRequested",
    });

    const next = load(queued.session, snapshot({ sessionId: 2 }));
    assert.deepEqual(next.commands, [{ mode: "area", type: "startSession" }]);
  });

  it("allows cancel while a commit is pending", () => {
    const first = load(createOverlaySession(), snapshot({ ready: false }));
    const queued = overlaySessionReducer(first.session, {
      rect: RECT,
      type: "commitRequested",
    });

    const cancelled = overlaySessionReducer(queued.session, {
      type: "cancelRequested",
    });
    assert.deepEqual(cancelled.commands, [
      { sessionId: 1, type: "sendCancel" },
    ]);

    const ready = load(cancelled.session, snapshot({ ready: true }));
    assert.deepEqual(ready.commands, []);
  });

  it("does not send cancel after a sent commit or twice", () => {
    const { session } = load(createOverlaySession(), snapshot());
    const sent = overlaySessionReducer(session, {
      rect: RECT,
      type: "commitRequested",
    });
    const afterCommit = overlaySessionReducer(sent.session, {
      type: "cancelRequested",
    });
    assert.deepEqual(afterCommit.commands, []);

    const first = overlaySessionReducer(session, { type: "cancelRequested" });
    assert.equal(first.commands.length, 1);
    const second = overlaySessionReducer(first.session, {
      type: "cancelRequested",
    });
    assert.deepEqual(second.commands, []);

    const failed = overlaySessionReducer(first.session, {
      type: "cancelFailed",
    });
    const retry = overlaySessionReducer(failed.session, {
      type: "cancelRequested",
    });
    assert.equal(retry.commands.length, 1);
  });

  it("clears pending commit and session on reset", () => {
    const first = load(createOverlaySession(), snapshot({ ready: false }));
    const queued = overlaySessionReducer(first.session, {
      rect: RECT,
      type: "commitRequested",
    });

    const reset = overlaySessionReducer(queued.session, { type: "reset" });
    assert.deepEqual(reset.commands, [{ type: "release" }]);
    assert.equal(reset.session.sessionId, null);
    assert.equal(reset.session.pending, null);
    assert.equal(reset.session.committed, false);
    assert.ok(reset.session.generation > queued.session.generation);

    const cancel = overlaySessionReducer(reset.session, {
      type: "cancelRequested",
    });
    assert.deepEqual(cancel.commands, []);
  });

  it("resets when the backend reports no state", () => {
    const first = load(createOverlaySession(), snapshot());
    const empty = load(first.session, null);

    assert.deepEqual(empty.commands, [{ type: "release" }]);
    assert.equal(empty.session.sessionId, null);
  });
});
