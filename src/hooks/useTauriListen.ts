import {
  type EventCallback,
  type EventName,
  listen,
} from "@tauri-apps/api/event";
import { useMount, useUnmount } from "ahooks";
import { useRef } from "react";
import { log } from "@/utils/log";

export const useTauriListen = <T>(
  event: EventName,
  handler: EventCallback<T>,
) => {
  const unlistenRef = useRef<(() => void) | undefined>(void 0);
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useMount(async () => {
    try {
      unlistenRef.current = await listen<T>(event, (payload) => {
        handlerRef.current(payload);
      });
    } catch (error) {
      log.error(`Failed to listen tauri event: ${event}`, error);
    }
  });

  useUnmount(() => {
    unlistenRef.current?.();
  });
};
