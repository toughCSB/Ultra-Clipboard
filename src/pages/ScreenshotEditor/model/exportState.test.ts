import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  createExportState,
  type ExportOutcome,
  hasUnexportedChanges,
  recordExport,
} from "./exportState";

type Doc = { id: string };

const original: Doc = { id: "original" };
const edited: Doc = { id: "edited" };

const outcome = (
  patch: Partial<ExportOutcome<Doc>> = {},
): ExportOutcome<Doc> => {
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
    assert.equal(
      hasUnexportedChanges(createExportState<Doc>(null), null),
      false,
    );
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
    const newer: Doc = { id: "newer" };
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
