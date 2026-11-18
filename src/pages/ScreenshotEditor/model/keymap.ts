import { type ArrowKey, isArrowKey } from "@/utils/pixelRect";
import { type EditorTool, toolByCode } from "./tools";

/** The keyboard event fields the editor shortcuts depend on. */
export interface KeyInput {
  altKey: boolean;
  code: string;
  ctrlKey: boolean;
  key: string;
  metaKey: boolean;
  repeat: boolean;
  shiftKey: boolean;
}

/** Where the key was pressed, as far as shortcut precedence cares. */
export interface KeyTarget {
  inDialog: boolean;
  /** A text input, textarea, contenteditable or the tool options panel. */
  inField: boolean;
}

/** `translation` and `textSelection` are exclusive overlays on the edit mode. */
export type KeyMode = "edit" | "textSelection" | "translation";

export interface KeyContext {
  dragActive: boolean;
  hasDocument: boolean;
  hasSelectedId: boolean;
  /** The selected id resolves to a shape in the document. */
  hasSelectedShape: boolean;
  hasSelection: boolean;
  keyModalOpen: boolean;
  mode: KeyMode;
  target: KeyTarget;
  textEditing: boolean;
  tool: EditorTool;
}

export type EditorCommand =
  | { type: "actualSize" }
  | { type: "cancelDrag" }
  | { type: "clearSelection" }
  | { type: "close" }
  | { type: "copy" }
  | { type: "copyAndClose" }
  | { type: "copyColor" }
  | { type: "copyImageText" }
  | { type: "crop" }
  | { type: "deleteShape" }
  | { type: "deselectShape" }
  | { type: "fit" }
  | { type: "holdPan" }
  | { type: "holdZoom" }
  | { type: "leaveTextSelection" }
  | { type: "leaveTranslation" }
  | { type: "ocr" }
  | { type: "paste" }
  | { type: "redo" }
  | { type: "save" }
  | { type: "selectTool"; tool: EditorTool }
  | { type: "undo" }
  | { type: "zoom"; direction: "in" | "out" }
  | { type: "nudgeShape"; key: ArrowKey; step: number }
  | { type: "nudgeSelection"; key: ArrowKey; step: number; extend: boolean };

export interface KeyAction {
  command: EditorCommand;
  preventDefault: boolean;
}

const act = (command: EditorCommand, preventDefault = true): KeyAction => {
  return { command, preventDefault };
};

const isLeaveKey = (input: KeyInput) => {
  return (
    input.key === "Escape" ||
    input.key === "Backspace" ||
    input.key === "BrowserBack" ||
    (input.altKey && input.key === "ArrowLeft")
  );
};

const zoomCommand = (code: string): EditorCommand | null => {
  switch (code) {
    case "Digit0":
    case "Numpad0":
      return { type: "fit" };
    case "Digit1":
    case "Numpad1":
      return { type: "actualSize" };
    case "Equal":
    case "NumpadAdd":
      return { direction: "in", type: "zoom" };
    case "Minus":
    case "NumpadSubtract":
      return { direction: "out", type: "zoom" };
    default:
      return null;
  }
};

const modifiedKey = (input: KeyInput, context: KeyContext) => {
  if (context.hasSelection && isArrowKey(input.key)) {
    return act({
      extend: true,
      key: input.key,
      step: input.shiftKey ? 10 : 1,
      type: "nudgeSelection",
    });
  }

  switch (input.code) {
    case "KeyC":
      return act({ type: "copy" });
    case "KeyS":
      return act({ type: "save" });
    case "KeyV":
      return act({ type: "paste" });
    case "KeyZ":
      return act({ type: input.shiftKey ? "redo" : "undo" });
    case "KeyY":
      return act({ type: "redo" });
    case "KeyW":
      return act({ type: "close" });
    case "Enter":
      return act({ type: "copyAndClose" });
    case "KeyO":
      return input.shiftKey ? act({ type: "ocr" }) : null;
  }

  const zoom = zoomCommand(input.code);

  return zoom ? act(zoom) : null;
};

/** The keys of an overlay mode: how to leave it and its Ctrl/Cmd shortcuts. */
interface OverlayKeys {
  leave: EditorCommand;
  shortcuts: ReadonlyMap<string, EditorCommand>;
}

const overlayKeys: Record<Exclude<KeyMode, "edit">, OverlayKeys> = {
  textSelection: {
    leave: { type: "leaveTextSelection" },
    shortcuts: new Map<string, EditorCommand>([
      ["KeyC", { type: "copyImageText" }],
      ["KeyS", { type: "save" }],
      ["KeyW", { type: "close" }],
    ]),
  },
  translation: {
    leave: { type: "leaveTranslation" },
    shortcuts: new Map<string, EditorCommand>([
      ["Enter", { type: "copyAndClose" }],
      ["KeyC", { type: "copy" }],
      ["KeyS", { type: "save" }],
      ["KeyW", { type: "close" }],
    ]),
  },
};

const overlayKey = (
  keys: OverlayKeys,
  input: KeyInput,
  context: KeyContext,
) => {
  if (isLeaveKey(input)) return act(keys.leave);
  if (!(input.ctrlKey || input.metaKey)) return null;

  const shortcut = keys.shortcuts.get(input.code);
  if (shortcut) return act(shortcut);

  const zoom = context.hasDocument ? zoomCommand(input.code) : null;

  return zoom ? act(zoom) : null;
};

const editKey = (input: KeyInput, context: KeyContext) => {
  if (!context.hasDocument || context.textEditing) return null;
  if (context.target.inField) return null;

  const key = input.key;

  if (key === "Tab") return act({ type: "copyColor" });
  if (input.code === "Space") return act({ type: "holdPan" });
  if (input.ctrlKey || input.metaKey) return modifiedKey(input, context);
  if (input.altKey) return null;
  if (input.code === "KeyZ") return act({ type: "holdZoom" }, false);

  if (key === "Delete" || key === "Backspace") {
    return context.hasSelectedId ? act({ type: "deleteShape" }) : null;
  }

  if (key === "Enter") return act({ type: "crop" });

  if (key === "Escape") {
    if (context.hasSelectedId) return act({ type: "deselectShape" }, false);
    if (context.hasSelection) return act({ type: "clearSelection" }, false);
    if (context.tool !== "select") {
      return act({ tool: "select", type: "selectTool" }, false);
    }

    return null;
  }

  if (isArrowKey(key)) {
    const step = input.shiftKey ? 10 : 1;

    if (context.hasSelectedShape) {
      return act({ key, step, type: "nudgeShape" });
    }
    if (context.hasSelection) {
      return act({ extend: false, key, step, type: "nudgeSelection" });
    }

    return null;
  }

  if (input.repeat) return null;

  const definition = toolByCode(input.code);

  return definition ? act({ tool: definition.id, type: "selectTool" }) : null;
};

/**
 * Maps one key press to an editor command, or null when the editor ignores it.
 * Precedence: drag cancel, key modal, translation dialogs, then the mode's own
 * keys. Ctrl and Cmd are interchangeable, so one table serves both platforms.
 */
export const resolveKey = (
  input: KeyInput,
  context: KeyContext,
): KeyAction | null => {
  if (context.dragActive && isLeaveKey(input)) {
    return act({ type: "cancelDrag" });
  }
  if (context.keyModalOpen) return null;
  if (context.mode === "edit") return editKey(input, context);
  if (context.mode === "translation" && context.target.inDialog) return null;

  return overlayKey(overlayKeys[context.mode], input, context);
};
