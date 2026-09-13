/** Frontend logging entry point shared by plugin logging and development console output. */
import {
  debug as pluginDebug,
  error as pluginError,
  info as pluginInfo,
  warn as pluginWarn,
} from "@tauri-apps/plugin-log";

import { isDev } from "./is";

type Payload = unknown;
type Level = "debug" | "info" | "warn" | "error";
/** Serialize a message and optional payload as one searchable log line. */
function format(message: string, payload?: Payload): string {
  if (payload === void 0) return message;

  if (payload instanceof Error) {
    return `${message}: ${payload.stack ?? payload.message}`;
  }

  try {
    return `${message}: ${JSON.stringify(payload)}`;
  } catch {
    return `${message}: ${String(payload)}`;
  }
}
/** Write logs to the browser console only in development. */
function devConsole(level: Level, message: string, payload?: Payload): void {
  if (!isDev) return;

  const args: unknown[] = payload === void 0 ? [message] : [message, payload];

  // biome-ignore lint/suspicious/noConsole: dev-only console mirror for log channel
  console[level](...args);
}

async function safe(
  fn: (msg: string) => Promise<void>,
  msg: string,
): Promise<void> {
  try {
    await fn(msg);
  } catch {
    // The IPC channel may not be ready during startup, so keep a console fallback.
    // biome-ignore lint/suspicious/noConsole: log channel fallback
    console.error("[log fallback]", msg);
  }
}

export const log = {
  debug: (message: string, payload?: Payload) => {
    devConsole("debug", message, payload);
    safe(pluginDebug, format(message, payload));
  },
  error: (message: string, payload?: Payload) => {
    devConsole("error", message, payload);
    safe(pluginError, format(message, payload));
  },
  info: (message: string, payload?: Payload) => {
    devConsole("info", message, payload);
    safe(pluginInfo, format(message, payload));
  },
  warn: (message: string, payload?: Payload) => {
    devConsole("warn", message, payload);
    safe(pluginWarn, format(message, payload));
  },
};
