import type { ClipboardPreviewPayload } from "@/commands";
import { PREVIEW_CACHE_LIMIT } from "./constants";
/** Read the latest payload for an item from the LRU cache and refresh its order. */
export function readCachedPayload(
  cache: Map<string, ClipboardPreviewPayload>,
  itemId: string,
  redactSecrets: boolean,
) {
  const keyPrefix = `${itemId}:`;
  const keySuffix = `:${redactSecrets ? "redacted" : "full"}`;
  const key = [...cache.keys()].find((entryKey) => {
    return entryKey.startsWith(keyPrefix) && entryKey.endsWith(keySuffix);
  });

  if (!key) return null;

  const cached = cache.get(key) ?? null;
  if (cached) {
    cache.delete(key);
    cache.set(key, cached);
  }

  return cached;
}
/** Cache a recent payload, binding its key to `updatedAt` to avoid stale reuse. */
export function writeCachedPayload(
  cache: Map<string, ClipboardPreviewPayload>,
  nextPayload: ClipboardPreviewPayload,
  redactSecrets: boolean,
) {
  cache.set(cacheKey(nextPayload, redactSecrets), nextPayload);

  while (cache.size > PREVIEW_CACHE_LIMIT) {
    const [oldestKey] = cache.keys();
    if (!oldestKey) return;

    cache.delete(oldestKey);
  }
}
/** Build the cache key for a preview payload. */
export function cacheKey(
  payload: ClipboardPreviewPayload,
  redactSecrets = payload.isSensitive,
) {
  return `${payload.id}:${payload.updatedAt}:${redactSecrets ? "redacted" : "full"}`;
}
