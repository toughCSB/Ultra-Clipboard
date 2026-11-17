import type {
  ScreenshotExportResult,
  ScreenshotTranslationBlock,
} from "@/commands";
import type { PixelRect } from "@/utils/pixelRect";
import type { TranslationLayer } from "../render/translation";
import type { EditorDocument } from "./document";

export type TranslationPhase =
  | "checkingKey"
  | "needsKey"
  | "translating"
  | "ready"
  | "failed";

/** One translation attempt on a fixed document and selection; every step returns a new value. */
export interface TranslationSession {
  area: PixelRect;
  blocks: readonly ScreenshotTranslationBlock[];
  document: EditorDocument;
  layer: TranslationLayer | null;
  phase: TranslationPhase;
  selection: PixelRect | null;
  showOriginal: boolean;
}

/** What a started step captured; the step is dropped when the editor no longer matches it. */
export interface TranslationToken {
  document: EditorDocument;
  generation: number;
  selection: PixelRect | null;
}

export interface TranslationEditorState extends TranslationToken {
  closing: boolean;
}

export type TranslationExportOutcome = Pick<
  ScreenshotExportResult,
  "translationBlocks" | "translationCancelled"
>;

export type TranslationResolution =
  | { kind: "cancelled" }
  | { kind: "blocks"; blocks: readonly ScreenshotTranslationBlock[] };

/**
 * Request IDs are positive safe integers that only grow, which is what the
 * backend registry accepts per window.
 */
export const createTranslationRequestIds = (now: () => number = Date.now) => {
  let last = 0;

  return {
    next() {
      const next = Math.max(now() * 1000, last + 1);
      if (!Number.isSafeInteger(next) || next <= 0) {
        throw new Error("translation request id is not a safe integer");
      }
      last = next;

      return next;
    },
  };
};

/** The single rule for ignoring an async result: it belongs to an old request, document, selection or a closing window. */
export const isTranslationStale = (
  token: TranslationToken,
  current: TranslationEditorState,
) => {
  return (
    current.closing ||
    token.generation !== current.generation ||
    token.document !== current.document ||
    token.selection !== current.selection
  );
};

export const startKeyCheck = (
  document: EditorDocument,
  selection: PixelRect | null,
  area: PixelRect,
): TranslationSession => {
  return {
    area,
    blocks: [],
    document,
    layer: null,
    phase: "checkingKey",
    selection,
    showOriginal: false,
  };
};

export const requireKey = (session: TranslationSession): TranslationSession => {
  return { ...session, phase: "needsKey" };
};

export const startTranslating = (
  document: EditorDocument,
  selection: PixelRect | null,
  area: PixelRect,
): TranslationSession => {
  return { ...startKeyCheck(document, selection, area), phase: "translating" };
};

export const finishTranslation = (
  session: TranslationSession,
  blocks: readonly ScreenshotTranslationBlock[],
  layer: TranslationLayer,
): TranslationSession => {
  return { ...session, blocks, layer, phase: "ready" };
};

export const failTranslation = (
  session: TranslationSession,
): TranslationSession => {
  return { ...session, phase: "failed" };
};

export const toggleOriginal = (
  session: TranslationSession,
): TranslationSession => {
  if (isLoading(session)) return session;

  return { ...session, showOriginal: !session.showOriginal };
};

/** Checking the key, waiting for one, and translating all keep the controls busy. */
export const isLoading = (session: TranslationSession) => {
  return (
    session.phase === "checkingKey" ||
    session.phase === "needsKey" ||
    session.phase === "translating"
  );
};

/** A translation waiting on the key check or on a saved key can still be started. */
export const isAwaitingKey = (session: TranslationSession | null) => {
  return session?.phase === "checkingKey" || session?.phase === "needsKey";
};

export const activeLayer = (session: TranslationSession | null) => {
  return session && !session.showOriginal ? session.layer : null;
};

export const translatedText = (session: TranslationSession) => {
  return session.blocks
    .map((block) => block.translatedText)
    .filter((line) => line.trim().length > 0)
    .join("\n");
};

/** A cancelled request is not "no text found"; only empty blocks mean that. */
export const resolveTranslation = (
  result: TranslationExportOutcome,
): TranslationResolution => {
  if (result.translationCancelled) return { kind: "cancelled" };

  return { blocks: result.translationBlocks ?? [], kind: "blocks" };
};

/** After a cancelled request the editor returns to a failed attempt, else to idle. */
export const settleCancelled = (previous: TranslationSession | null) => {
  return previous?.phase === "failed" ? previous : null;
};
