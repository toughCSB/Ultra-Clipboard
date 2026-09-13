import { useEffect, useRef, useState } from "react";
import { useSnapshot } from "valtio";
import {
  type ClipboardPreviewPayload,
  type ClipboardPreviewState,
  getClipboardPreviewPayload,
} from "@/commands";
import { settingsState } from "@/stores/settings";
import { log } from "@/utils/log";
import { readCachedPayload, writeCachedPayload } from "./cache";
import { PREVIEW_EXIT_ANIMATION_MS } from "./constants";
/** Keep the last preview state during exit animation, then clear the render tree. */
export function usePreviewRenderState(
  previewState: ClipboardPreviewState | null,
) {
  const [renderState, setRenderState] = useState<ClipboardPreviewState | null>(
    null,
  );
  const exitTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (exitTimerRef.current !== null) {
      window.clearTimeout(exitTimerRef.current);
      exitTimerRef.current = null;
    }

    if (previewState) {
      setRenderState(previewState);
      return;
    }

    exitTimerRef.current = window.setTimeout(() => {
      setRenderState(null);
      exitTimerRef.current = null;
    }, PREVIEW_EXIT_ANIMATION_MS);

    return () => {
      if (exitTimerRef.current === null) return;

      window.clearTimeout(exitTimerRef.current);
      exitTimerRef.current = null;
    };
  }, [previewState]);

  return renderState;
}
/** Load a payload by preview state and reuse recent contents from the LRU cache. */
export function usePreviewPayload(
  previewState: ClipboardPreviewState | null,
  resetToken = 0,
) {
  const [payload, setPayload] = useState<ClipboardPreviewPayload | null>(null);
  const [loadingItemId, setLoadingItemId] = useState<string | null>(null);
  const latestRequestIdRef = useRef(0);
  const cacheRef = useRef(new Map<string, ClipboardPreviewPayload>());
  const { clipboard } = useSnapshot(settingsState);
  const redactSecrets = clipboard.sensitive.redactSecrets;

  // This trigger clears the cache before the preview window is destroyed.
  // biome-ignore lint/correctness/useExhaustiveDependencies: resetToken is a destruction-trigger cache clear.
  useEffect(() => {
    cacheRef.current.clear();
    latestRequestIdRef.current += 1;
    setLoadingItemId(null);
    setPayload(null);
  }, [resetToken]);

  useEffect(() => {
    if (!previewState) {
      setLoadingItemId(null);
      return;
    }

    const state = previewState;
    let cancelled = false;
    latestRequestIdRef.current = state.requestId;

    const cached = readCachedPayload(
      cacheRef.current,
      state.itemId,
      redactSecrets,
    );
    if (cached) {
      setPayload(cached);
      setLoadingItemId(null);
      return;
    }

    setLoadingItemId(state.itemId);

    async function loadPayload() {
      try {
        const nextPayload = await getClipboardPreviewPayload(state.itemId);

        if (cancelled || latestRequestIdRef.current !== state.requestId) {
          return;
        }

        if (!nextPayload) {
          setPayload(null);
          setLoadingItemId(null);
          return;
        }

        writeCachedPayload(cacheRef.current, nextPayload, redactSecrets);
        setPayload(nextPayload);
        setLoadingItemId(null);
      } catch (error) {
        if (cancelled || latestRequestIdRef.current !== state.requestId) {
          return;
        }

        log.error("load preview payload failed", error);
        setLoadingItemId(null);
      }
    }

    void loadPayload();

    return () => {
      cancelled = true;
    };
  }, [previewState, redactSecrets]);

  return { loadingItemId, payload };
}
