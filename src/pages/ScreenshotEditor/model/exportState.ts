export interface ExportState<D> {
  /** The document the user last got out of the editor, or null before load. */
  exported: D | null;
}

export interface ExportOutcome<D> {
  action: "copy" | "ocr" | "pin" | "save" | "saveAs";
  /** The document as it is now, which may have moved on during the export. */
  current: D | null;
  /** The document that was rendered. */
  document: D;
  savedPath: string | null;
  translated: boolean;
}

export const createExportState = <D>(exported: D | null): ExportState<D> => {
  return { exported };
};

export const hasUnexportedChanges = <D>(
  state: ExportState<D>,
  present: D | null,
) => {
  return present !== null && state.exported !== present;
};

/**
 * Text recognition, a cancelled Save As, a translated render and a document
 * edited during the export all leave the state as it was.
 */
export const recordExport = <D>(
  state: ExportState<D>,
  outcome: ExportOutcome<D>,
): ExportState<D> => {
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
