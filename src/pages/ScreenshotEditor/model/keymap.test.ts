import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  type KeyContext,
  type KeyInput,
  type KeyTarget,
  resolveKey,
} from "./keymap";

const press = (patch: Partial<KeyInput>): KeyInput => {
  return {
    altKey: false,
    code: "",
    ctrlKey: false,
    key: "",
    metaKey: false,
    repeat: false,
    shiftKey: false,
    ...patch,
  };
};

const context = (patch: Partial<KeyContext> = {}): KeyContext => {
  return {
    dragActive: false,
    hasDocument: true,
    hasSelectedId: false,
    hasSelectedShape: false,
    hasSelection: false,
    keyModalOpen: false,
    mode: "edit",
    target: { inDialog: false, inField: false },
    textEditing: false,
    tool: "select",
    ...patch,
  };
};

const command = (input: KeyInput, ctx: KeyContext = context()) => {
  return resolveKey(input, ctx)?.command ?? null;
};

/** Both platform chords: Windows Ctrl and macOS Cmd. */
const chords: Array<Partial<KeyInput>> = [{ ctrlKey: true }, { metaKey: true }];

const leavers: Array<Partial<KeyInput>> = [
  { key: "Escape" },
  { key: "Backspace" },
  { key: "BrowserBack" },
  { altKey: true, key: "ArrowLeft" },
];

describe("screenshot editor keymap: modified shortcuts in edit mode", () => {
  const table: Array<[string, Partial<KeyInput>, unknown]> = [
    ["KeyC", {}, { type: "copy" }],
    ["KeyS", {}, { type: "save" }],
    ["KeyV", {}, { type: "paste" }],
    ["KeyZ", {}, { type: "undo" }],
    ["KeyZ", { shiftKey: true }, { type: "redo" }],
    ["KeyY", {}, { type: "redo" }],
    ["KeyW", {}, { type: "close" }],
    ["Enter", {}, { type: "copyAndClose" }],
    ["KeyO", { shiftKey: true }, { type: "ocr" }],
    ["Digit0", {}, { type: "fit" }],
    ["Numpad0", {}, { type: "fit" }],
    ["Digit1", {}, { type: "actualSize" }],
    ["Numpad1", {}, { type: "actualSize" }],
    ["Equal", {}, { direction: "in", type: "zoom" }],
    ["NumpadAdd", {}, { direction: "in", type: "zoom" }],
    ["Minus", {}, { direction: "out", type: "zoom" }],
    ["NumpadSubtract", {}, { direction: "out", type: "zoom" }],
  ];

  for (const chord of chords) {
    const platform = chord.metaKey ? "macOS Cmd" : "Windows Ctrl";
    for (const [code, extra, expected] of table) {
      const shift = extra.shiftKey ? "Shift+" : "";
      it(`${platform}+${shift}${code} maps to ${JSON.stringify(expected)}`, () => {
        const action = resolveKey(
          press({ code, ...chord, ...extra }),
          context(),
        );
        assert.deepEqual(action, { command: expected, preventDefault: true });
      });
    }
  }

  it("ignores Ctrl+O without Shift and unknown chords", () => {
    assert.equal(
      resolveKey(press({ code: "KeyO", ctrlKey: true }), context()),
      null,
    );
    assert.equal(
      resolveKey(press({ code: "KeyQ", ctrlKey: true }), context()),
      null,
    );
  });

  it("nudges the crop selection with Ctrl+arrow, 10 px with Shift, extending the edge", () => {
    const ctx = context({ hasSelection: true });
    assert.deepEqual(
      resolveKey(
        press({ code: "ArrowLeft", ctrlKey: true, key: "ArrowLeft" }),
        ctx,
      ),
      {
        command: {
          extend: true,
          key: "ArrowLeft",
          step: 1,
          type: "nudgeSelection",
        },
        preventDefault: true,
      },
    );
    assert.deepEqual(
      command(
        press({
          code: "ArrowDown",
          key: "ArrowDown",
          metaKey: true,
          shiftKey: true,
        }),
        ctx,
      ),
      { extend: true, key: "ArrowDown", step: 10, type: "nudgeSelection" },
    );
  });

  it("does nothing for Ctrl+arrow without a selection", () => {
    assert.equal(
      resolveKey(
        press({ code: "ArrowLeft", ctrlKey: true, key: "ArrowLeft" }),
        context(),
      ),
      null,
    );
  });

  it("runs Ctrl shortcuts before tool shortcuts and ignores Alt-only keys", () => {
    assert.equal(
      command(press({ altKey: true, code: "KeyR", key: "r" })),
      null,
    );
    assert.deepEqual(
      command(press({ code: "KeyC", ctrlKey: true, key: "c" })),
      {
        type: "copy",
      },
    );
  });

  it("gives Space precedence over Ctrl chords", () => {
    assert.deepEqual(
      command(press({ code: "Space", ctrlKey: true, key: " " })),
      { type: "holdPan" },
    );
  });
});

describe("screenshot editor keymap: plain keys in edit mode", () => {
  it("copies the hover colour on Tab", () => {
    assert.deepEqual(resolveKey(press({ key: "Tab" }), context()), {
      command: { type: "copyColor" },
      preventDefault: true,
    });
  });

  it("holds pan on Space and zoom on Z (without preventing Z)", () => {
    assert.deepEqual(
      resolveKey(press({ code: "Space", key: " " }), context()),
      { command: { type: "holdPan" }, preventDefault: true },
    );
    assert.deepEqual(resolveKey(press({ code: "KeyZ", key: "z" }), context()), {
      command: { type: "holdZoom" },
      preventDefault: false,
    });
  });

  it("deletes only when a shape id is selected", () => {
    for (const key of ["Delete", "Backspace"]) {
      assert.equal(resolveKey(press({ key }), context()), null);
      assert.deepEqual(
        resolveKey(press({ key }), context({ hasSelectedId: true })),
        { command: { type: "deleteShape" }, preventDefault: true },
      );
    }
  });

  it("crops on Enter", () => {
    assert.deepEqual(
      resolveKey(press({ code: "Enter", key: "Enter" }), context()),
      { command: { type: "crop" }, preventDefault: true },
    );
  });

  it("peels Escape in order: shape, selection, tool, nothing", () => {
    const escapeKey = press({ code: "Escape", key: "Escape" });
    assert.deepEqual(
      resolveKey(
        escapeKey,
        context({ hasSelectedId: true, hasSelection: true, tool: "rect" }),
      ),
      { command: { type: "deselectShape" }, preventDefault: false },
    );
    assert.deepEqual(
      resolveKey(escapeKey, context({ hasSelection: true, tool: "rect" })),
      { command: { type: "clearSelection" }, preventDefault: false },
    );
    assert.deepEqual(resolveKey(escapeKey, context({ tool: "rect" })), {
      command: { tool: "select", type: "selectTool" },
      preventDefault: false,
    });
    assert.equal(resolveKey(escapeKey, context()), null);
  });

  it("nudges the selected shape before the selection, 1 px or 10 px with Shift", () => {
    const arrow = press({ code: "ArrowUp", key: "ArrowUp" });
    const ctx = context({ hasSelectedShape: true, hasSelection: true });
    assert.deepEqual(resolveKey(arrow, ctx), {
      command: { key: "ArrowUp", step: 1, type: "nudgeShape" },
      preventDefault: true,
    });
    assert.deepEqual(command({ ...arrow, shiftKey: true }, ctx), {
      key: "ArrowUp",
      step: 10,
      type: "nudgeShape",
    });
  });

  it("nudges the selection when no shape is selected, and nothing otherwise", () => {
    const arrow = press({
      code: "ArrowRight",
      key: "ArrowRight",
      shiftKey: true,
    });
    assert.deepEqual(resolveKey(arrow, context({ hasSelection: true })), {
      command: {
        extend: false,
        key: "ArrowRight",
        step: 10,
        type: "nudgeSelection",
      },
      preventDefault: true,
    });
    assert.equal(resolveKey(arrow, context()), null);
  });

  it("switches tool by key code, ignoring auto-repeat", () => {
    const rect = press({ code: "KeyR", key: "r" });
    const action = resolveKey(rect, context());
    assert.equal(action?.command.type, "selectTool");
    assert.equal(action?.preventDefault, true);
    assert.equal(resolveKey({ ...rect, repeat: true }, context()), null);
    assert.equal(
      resolveKey(press({ code: "KeyQ", key: "q" }), context()),
      null,
    );
  });

  it("ignores everything without a document or while editing text", () => {
    assert.equal(
      resolveKey(press({ key: "Tab" }), context({ hasDocument: false })),
      null,
    );
    assert.equal(
      resolveKey(press({ key: "Tab" }), context({ textEditing: true })),
      null,
    );
    assert.equal(
      resolveKey(
        press({ code: "KeyC", ctrlKey: true }),
        context({ textEditing: true }),
      ),
      null,
    );
  });

  it("ignores keys typed into inputs and the options panel, even Ctrl shortcuts", () => {
    const target: KeyTarget = { inDialog: false, inField: true };
    assert.equal(resolveKey(press({ key: "Tab" }), context({ target })), null);
    assert.equal(
      resolveKey(press({ code: "KeyC", ctrlKey: true }), context({ target })),
      null,
    );
    assert.equal(
      resolveKey(press({ code: "KeyR", key: "r" }), context({ target })),
      null,
    );
  });
});

describe("screenshot editor keymap: drag gesture and key modal", () => {
  it("cancels an active drag gesture before every mode", () => {
    for (const mode of ["edit", "textSelection", "translation"] as const) {
      for (const leaver of leavers) {
        assert.deepEqual(
          resolveKey(
            press(leaver),
            context({ dragActive: true, hasSelectedId: true, mode }),
          ),
          { command: { type: "cancelDrag" }, preventDefault: true },
        );
      }
    }
    assert.deepEqual(
      resolveKey(
        press({ key: "Escape" }),
        context({ dragActive: true, keyModalOpen: true, textEditing: true }),
      ),
      { command: { type: "cancelDrag" }, preventDefault: true },
    );
  });

  it("does not cancel a drag for other keys", () => {
    assert.equal(
      command(press({ key: "ArrowLeft" }), context({ dragActive: true })),
      null,
    );
  });

  it("ignores all keys while the translation key modal is open", () => {
    for (const mode of ["edit", "textSelection", "translation"] as const) {
      assert.equal(
        resolveKey(
          press({ code: "KeyC", ctrlKey: true, key: "Escape" }),
          context({ keyModalOpen: true, mode }),
        ),
        null,
      );
    }
  });
});

describe("screenshot editor keymap: translation mode", () => {
  const ctx = context({ mode: "translation" });

  it("leaves translation on Escape, Backspace, BrowserBack and Alt+Left", () => {
    for (const leaver of leavers) {
      assert.deepEqual(resolveKey(press(leaver), ctx), {
        command: { type: "leaveTranslation" },
        preventDefault: true,
      });
    }
  });

  it("exports and closes through the allowed chords on both platforms", () => {
    const expected: Array<[string, unknown]> = [
      ["KeyC", { type: "copy" }],
      ["KeyS", { type: "save" }],
      ["KeyW", { type: "close" }],
      ["Enter", { type: "copyAndClose" }],
    ];
    for (const chord of chords) {
      for (const [code, want] of expected) {
        assert.deepEqual(resolveKey(press({ code, ...chord }), ctx), {
          command: want,
          preventDefault: true,
        });
      }
    }
  });

  it("still zooms with Ctrl but blocks other shortcuts and plain keys", () => {
    assert.deepEqual(command(press({ code: "Digit0", ctrlKey: true }), ctx), {
      type: "fit",
    });
    assert.deepEqual(command(press({ code: "Minus", metaKey: true }), ctx), {
      direction: "out",
      type: "zoom",
    });
    assert.equal(
      command(press({ code: "Digit0", ctrlKey: true }), {
        ...ctx,
        hasDocument: false,
      }),
      null,
    );
    for (const code of ["KeyV", "KeyZ", "KeyY"]) {
      assert.equal(command(press({ code, ctrlKey: true }), ctx), null);
    }
    assert.equal(command(press({ code: "KeyR", key: "r" }), ctx), null);
    assert.equal(command(press({ key: "Tab" }), ctx), null);
  });

  it("lets dialogs inside translation handle their own keys", () => {
    const inDialog = { ...ctx, target: { inDialog: true, inField: false } };
    assert.equal(resolveKey(press({ key: "Escape" }), inDialog), null);
    assert.equal(
      resolveKey(press({ code: "KeyC", ctrlKey: true }), inDialog),
      null,
    );
  });

  it("applies the dialog bypass only to translation mode", () => {
    const inDialog = { inDialog: true, inField: false };
    assert.deepEqual(
      command(
        press({ code: "KeyC", ctrlKey: true }),
        context({ target: inDialog }),
      ),
      { type: "copy" },
    );
  });
});

describe("screenshot editor keymap: text selection mode", () => {
  const ctx = context({ mode: "textSelection" });

  it("leaves text selection on Escape, Backspace, BrowserBack and Alt+Left", () => {
    for (const leaver of leavers) {
      assert.deepEqual(resolveKey(press(leaver), ctx), {
        command: { type: "leaveTextSelection" },
        preventDefault: true,
      });
    }
  });

  it("copies image text, saves and closes with Ctrl or Cmd", () => {
    for (const chord of chords) {
      assert.deepEqual(command(press({ code: "KeyC", ...chord }), ctx), {
        type: "copyImageText",
      });
      assert.deepEqual(command(press({ code: "KeyS", ...chord }), ctx), {
        type: "save",
      });
      assert.deepEqual(command(press({ code: "KeyW", ...chord }), ctx), {
        type: "close",
      });
    }
  });

  it("still zooms but blocks Enter, paste, undo and plain keys", () => {
    assert.deepEqual(command(press({ code: "Equal", ctrlKey: true }), ctx), {
      direction: "in",
      type: "zoom",
    });
    for (const code of ["Enter", "KeyV", "KeyZ", "KeyY"]) {
      assert.equal(command(press({ code, ctrlKey: true }), ctx), null);
    }
    assert.equal(command(press({ code: "Space", key: " " }), ctx), null);
    assert.equal(command(press({ code: "KeyR", key: "r" }), ctx), null);
  });

  it("does not look at text inputs in this mode", () => {
    assert.deepEqual(
      command(press({ code: "KeyC", ctrlKey: true }), {
        ...ctx,
        target: { inDialog: false, inField: true },
      }),
      { type: "copyImageText" },
    );
  });
});
