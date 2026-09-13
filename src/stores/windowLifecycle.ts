import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { proxy } from "valtio";

import { TAURI_EVENT } from "@/constants/events";
import { log } from "@/utils/log";

/** Lifecycle stages mirror Rust `LifecyclePhase` in camelCase. */
export type LifecyclePhase =
  | "notCreated"
  | "created"
  | "ready"
  | "visible"
  | "hiddenWarm"
  | "dormant"
  | "destroyPending"
  | "destroyed";

/** Payload emitted by the Rust `window://lifecycle` event. */
interface LifecyclePayload {
  label: string;
  phase: LifecyclePhase;
  generation: number;
  reason: string;
  visible: boolean;
}

/** Lifecycle mirror for the current WebView. Rust is the source of truth. */
export const windowLifecycleState = proxy<{
  phase: LifecyclePhase;
  visible: boolean;
  generation: number;
}>({
  generation: 0,
  phase: "created",
  visible: false,
});

const currentLabel = getCurrentWebviewWindow().label;

/** Subscribe once to lifecycle updates for the current window. */
export const windowLifecycleReady: Promise<void> = (async () => {
  try {
    await listen<LifecyclePayload>(TAURI_EVENT.WINDOW_LIFECYCLE, (event) => {
      const { generation, label, phase, visible } = event.payload;

      if (label !== currentLabel) return;

      windowLifecycleState.generation = generation;
      windowLifecycleState.phase = phase;
      windowLifecycleState.visible = visible;
    });
  } catch (error) {
    log.error("Failed to listen window lifecycle event", error);
  }
})();
