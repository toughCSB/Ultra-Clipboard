import { useMemoizedFn, useUnmount } from "ahooks";
import { useRef, useState } from "react";
import {
  cancelScreenshotTranslation,
  exportScreenshot,
  getScreenshotTranslationStatus,
  type ScreenshotTranslationStatus,
} from "@/commands";
import { log } from "@/utils/log";
import type { PixelRect } from "@/utils/pixelRect";
import { documentBounds, type EditorDocument } from "../model/document";
import {
  activeLayer,
  createTranslationRequestIds,
  failTranslation,
  finishTranslation,
  isAwaitingKey,
  isTranslationStale,
  requireKey,
  resolveTranslation,
  settleCancelled,
  startKeyCheck,
  startTranslating,
  type TranslationSession,
  toggleOriginal,
} from "../model/translationSession";
import { createTranslationLayer } from "../render/translation";

interface TranslationSessionOptions {
  label: string;
  /** Renders the pixels of `area`, or null while the editor image is not loaded. */
  renderPixels: (
    document: EditorDocument,
    area: PixelRect,
  ) => Uint8Array | null;
  /** Reads the editor's live document, selection and closing flag for the staleness rule. */
  readEditor: () => {
    closing: boolean;
    document: EditorDocument | null;
    selection: PixelRect | null;
  };
  onBusyChange: (busy: boolean) => void;
  /** The active translated layer changed, so any prepared drag file is outdated. */
  onLayerChange: () => void;
  onKeyStatus: (status: ScreenshotTranslationStatus) => void;
  onNeedsKey: () => void;
}

/** Drives one translation session at a time and keeps its state in sync with the backend request. */
export const useTranslationSession = (options: TranslationSessionOptions) => {
  const [session, setSessionState] = useState<TranslationSession | null>(null);
  const sessionRef = useRef<TranslationSession | null>(null);
  const generationRef = useRef(0);
  const requestIdsRef = useRef(createTranslationRequestIds());
  const activeRequestRef = useRef<number | null>(null);
  const cancelRef = useRef<Promise<void> | null>(null);

  const setSession = (next: TranslationSession | null) => {
    sessionRef.current = next;
    setSessionState(next);
  };

  const isStale = (
    generation: number,
    document: EditorDocument,
    selection: PixelRect | null,
  ) => {
    const editor = options.readEditor();

    return (
      editor.document === null ||
      isTranslationStale(
        { document, generation, selection },
        {
          closing: editor.closing,
          document: editor.document,
          generation: generationRef.current,
          selection: editor.selection,
        },
      )
    );
  };

  const cancelActiveRequest = useMemoizedFn(() => {
    const requestId = activeRequestRef.current;
    if (requestId === null) return cancelRef.current;

    activeRequestRef.current = null;
    const cancelled = (async () => {
      try {
        await cancelScreenshotTranslation(options.label, requestId);
      } catch (error) {
        log.warn("cancel screenshot translation failed", error);
      }
    })();
    cancelRef.current = cancelled;

    return cancelled;
  });

  useUnmount(() => {
    generationRef.current += 1;
    void cancelActiveRequest();
  });

  const leave = useMemoizedFn(() => {
    if (!sessionRef.current) return;

    generationRef.current += 1;
    void cancelActiveRequest();
    setSession(null);
    options.onBusyChange(false);
    options.onLayerChange();
  });

  const start = useMemoizedFn(
    async (document: EditorDocument, selection: PixelRect | null) => {
      if (options.readEditor().closing) return;

      const generation = generationRef.current + 1;
      generationRef.current = generation;
      const area = selection ?? documentBounds(document);
      const previous = sessionRef.current;
      const translating = startTranslating(document, selection, area);
      setSession(translating);
      options.onBusyChange(true);

      try {
        if (cancelRef.current) {
          await cancelRef.current;
        }
        if (isStale(generation, document, selection)) return;

        const pixels = options.renderPixels(document, area);
        if (!pixels) {
          setSession(failTranslation(translating));
          return;
        }
        const requestId = requestIdsRef.current.next();
        activeRequestRef.current = requestId;
        const result = await exportScreenshot({
          action: "translate",
          area,
          label: options.label,
          pixels,
          requestId,
        });
        if (activeRequestRef.current === requestId) {
          activeRequestRef.current = null;
        }
        if (isStale(generation, document, selection)) return;

        const resolution = resolveTranslation(result);
        if (resolution.kind === "cancelled") {
          setSession(settleCancelled(previous));
          return;
        }

        const layer = createTranslationLayer(area, resolution.blocks, pixels);
        setSession(finishTranslation(translating, resolution.blocks, layer));
        options.onLayerChange();
      } catch (error) {
        if (generation !== generationRef.current) return;
        void cancelActiveRequest();
        setSession(failTranslation(translating));
        log.error("translate screenshot failed", error);
      } finally {
        if (generation === generationRef.current) {
          options.onBusyChange(false);
        }
      }
    },
  );

  /** Checks the key first; a missing key leaves the session waiting for the key dialog. */
  const begin = useMemoizedFn(
    async (document: EditorDocument, selection: PixelRect | null) => {
      const generation = generationRef.current + 1;
      generationRef.current = generation;
      const checking = startKeyCheck(
        document,
        selection,
        selection ?? documentBounds(document),
      );
      setSession(checking);
      options.onBusyChange(true);

      try {
        const status = await getScreenshotTranslationStatus();
        if (isStale(generation, document, selection)) return;

        options.onKeyStatus(status);
        if (!status.configured) {
          setSession(requireKey(checking));
          options.onNeedsKey();
          return;
        }

        void start(document, selection);
      } catch (error) {
        if (generation === generationRef.current) {
          setSession(failTranslation(checking));
        }
        log.error("read screenshot translation status failed", error);
      } finally {
        if (generation === generationRef.current) {
          options.onBusyChange(false);
        }
      }
    },
  );

  /** Continues a translation that was waiting for a key, if the editor still matches it. */
  const resumeAfterKey = useMemoizedFn(() => {
    const pending = sessionRef.current;
    if (!pending || !isAwaitingKey(pending)) return;
    if (isStale(generationRef.current, pending.document, pending.selection)) {
      return;
    }

    void start(pending.document, pending.selection);
  });

  const retry = useMemoizedFn(() => {
    const current = sessionRef.current;
    if (current?.phase !== "failed") return;

    void start(current.document, current.selection);
  });

  const toggleShowOriginal = useMemoizedFn(() => {
    const current = sessionRef.current;
    if (!current) return;

    const next = toggleOriginal(current);
    if (next === current) return;

    setSession(next);
    options.onLayerChange();
  });

  const currentLayer = () => {
    return activeLayer(sessionRef.current);
  };

  const cancelSettled = () => {
    return cancelRef.current;
  };

  const current = () => {
    return sessionRef.current;
  };

  return {
    activeLayer: currentLayer,
    begin,
    cancelSettled,
    current,
    leave,
    resumeAfterKey,
    retry,
    session,
    toggleShowOriginal,
  };
};
