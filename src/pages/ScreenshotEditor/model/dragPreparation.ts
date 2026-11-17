import type { PixelRect } from "@/utils/pixelRect";
import type { TranslationLayer } from "../render/translation";
import type { EditorDocument } from "./document";

/** What a prepared drag file was rendered from. */
export interface DragIdentity {
  document: EditorDocument | null;
  selection: PixelRect | null;
  translation: TranslationLayer | null;
}

export const sameDragIdentity = (a: DragIdentity, b: DragIdentity) => {
  return (
    a.document === b.document &&
    a.selection === b.selection &&
    a.translation === b.translation
  );
};

interface Preparation extends DragIdentity {
  ready: Promise<boolean>;
}

/**
 * Remembers the drag file prepared for one identity. A new identity queues
 * behind the previous render and only counts as ready if the live identity
 * still matches before and after its own render.
 */
export const createDragPreparationCache = () => {
  let entry: Preparation | null = null;
  let generation = 0;

  return {
    invalidate() {
      entry = null;
      generation += 1;
    },
    prepare(
      wanted: DragIdentity,
      readLive: () => DragIdentity,
      render: () => Promise<boolean>,
      onError?: (error: unknown) => void,
    ) {
      if (entry && sameDragIdentity(entry, wanted)) return entry.ready;

      const previousReady = entry?.ready;
      const id = ++generation;
      const ready: Promise<boolean> = (async () => {
        if (previousReady) await previousReady;
        if (!sameDragIdentity(readLive(), wanted)) return false;

        try {
          const prepared = await render();

          return prepared && sameDragIdentity(readLive(), wanted);
        } catch (error) {
          onError?.(error);
          if (generation === id) entry = null;

          return false;
        }
      })();

      entry = { ...wanted, ready };

      return ready;
    },
  };
};
