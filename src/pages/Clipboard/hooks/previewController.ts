import type { RefObject } from "react";
import { closeClipboardPreview } from "@/commands";
import type { ClipboardItem } from "@/types/clipboard";
import type { PreviewHoverDelayMs } from "@/types/settings";
import { log } from "@/utils/log";

export const HOVER_DELAY_MS: Record<PreviewHoverDelayMs, number> = {
  ms300: 300,
  ms500: 500,
  ms1000: 1000,
};
export const HOVER_HIDE_BUFFER_MS = 240;

export type PreviewTrigger = "keyboard" | "hover";

export interface PreviewSession {
  itemId: string;
  trigger: PreviewTrigger;
}

export interface WindowVisibilityPayload {
  label: string;
  visible: boolean;
}

export interface UseClipboardPreviewControllerOptions {
  getActiveItem: () => ClipboardItem | null;
  itemElementMapRef: RefObject<Map<string, HTMLDivElement>>;
  onHoverSelect: (id: string) => void;
}

/** Recognize Space across modern and older WebKit key names. */
export function isSpaceKey(event: KeyboardEvent) {
  return (
    event.key === " " ||
    event.key === "Space" ||
    event.key === "Spacebar" ||
    event.code === "Space"
  );
}

/** Clear a pending hover-delay timer. */
export function clearHoverTimer(timerRef: { current: number | null }) {
  if (timerRef.current === null) return;

  window.clearTimeout(timerRef.current);
  timerRef.current = null;
}

/** Close preview on a non-critical path and log failures without surfacing them. */
export async function closeClipboardPreviewSilently(reason: string) {
  try {
    await closeClipboardPreview();
  } catch (error) {
    log.error("close clipboard preview failed", { error, reason });
  }
}
