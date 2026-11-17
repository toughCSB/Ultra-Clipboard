import type { ScreenshotExportAction } from "@/commands";
import type { EditorDocument } from "./document";

export interface ExportState {
  /** The document the user last got out of the editor, or null before load. */
  exported: EditorDocument | null;
}

export interface ExportOutcome {
  action: Extract<
    ScreenshotExportAction,
    "copy" | "ocr" | "pin" | "save" | "saveAs"
  >;
  /** The document as it is now, which may have moved on during the export. */
  current: EditorDocument | null;
  /** The document that was rendered. */
  document: EditorDocument;
  savedPath: string | null;
  translated: boolean;
}

export const createExportState = (
  exported: EditorDocument | null,
): ExportState => {
  return { exported };
};

export const hasUnexportedChanges = (
  state: ExportState,
  present: EditorDocument | null,
) => {
  return present !== null && state.exported !== present;
};

/**
 * Text recognition, a cancelled Save As, a translated render and a document
 * edited during the export all leave the state as it was.
 */
export const recordExport = (
  state: ExportState,
  outcome: ExportOutcome,
): ExportState => {
  if (outcome.action === "ocr") return state;
  if (
    (outcome.action === "save" || outcome.action === "saveAs") &&
    !outcome.savedPath
  ) {
    return state;
  }
  if (outcome.translated || outcome.current !== outcome.document) return state;

  return { exported: outcome.document };
};
