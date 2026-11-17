import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { createDocument } from "./document";
import {
  createExportState,
  type ExportOutcome,
  hasUnexportedChanges,
  recordExport,
} from "./exportState";

const crop = { height: 10, width: 10, x: 0, y: 0 };
const original = createDocument(crop);
const edited = createDocument(crop);

const outcome = (patch: Partial<ExportOutcome> = {}): ExportOutcome => {
  return {
    action: "copy",
    current: edited,
    document: edited,
    savedPath: null,
    translated: false,
    ...patch,
  };
};

describe("screenshot export state", () => {
  it("is clean for the freshly loaded document", () => {
    assert.equal(
      hasUnexportedChanges(createExportState(original), original),
      false,
    );
  });

  it("is dirty after an edit", () => {
    assert.equal(
      hasUnexportedChanges(createExportState(original), edited),
      true,
    );
  });

  it("is clean when there is no document", () => {
    assert.equal(hasUnexportedChanges(createExportState(null), null), false);
  });

  it("copy, pin, save and saveAs clear the dirty flag", () => {
    for (const [action, savedPath] of [
      ["copy", null],
      ["pin", null],
      ["save", "C:/shots/a.png"],
      ["saveAs", "C:/shots/b.png"],
    ] as const) {
      const next = recordExport(
        createExportState(original),
        outcome({ action, savedPath }),
      );
      assert.equal(hasUnexportedChanges(next, edited), false, action);
    }
  });

  it("a cancelled save or Save As keeps it dirty", () => {
    for (const action of ["save", "saveAs"] as const) {
      const next = recordExport(
        createExportState(original),
        outcome({ action, savedPath: null }),
      );
      assert.equal(hasUnexportedChanges(next, edited), true, action);
    }
  });

  it("a translated export keeps it dirty", () => {
    const next = recordExport(
      createExportState(original),
      outcome({ translated: true }),
    );
    assert.equal(hasUnexportedChanges(next, edited), true);
  });

  it("text recognition never clears it", () => {
    const next = recordExport(
      createExportState(original),
      outcome({ action: "ocr" }),
    );
    assert.equal(hasUnexportedChanges(next, edited), true);
  });

  it("an export of a document that was edited meanwhile keeps it dirty", () => {
    const newer = createDocument(crop);
    const next = recordExport(
      createExportState(original),
      outcome({ current: newer }),
    );
    assert.equal(hasUnexportedChanges(next, newer), true);
    assert.equal(next.exported, original);
  });

  it("returns a new state without mutating the previous one", () => {
    const state = createExportState(original);
    recordExport(state, outcome());
    assert.equal(hasUnexportedChanges(state, edited), true);
  });
});
