import { listen } from "@tauri-apps/api/event";
import { proxy } from "valtio";

import {
  getSettings,
  resetSettings as invokeResetSettings,
  updateSettings as invokeUpdateSettings,
} from "@/commands";
import { TAURI_EVENT } from "@/constants/events";
import type { Settings, SettingsPatch } from "@/types/settings";

/** Local settings mirror. Rust remains the source of truth and broadcasts updates. */
export const settingsState = proxy<Settings>({} as Settings);

/** Subscribe to Rust updates and load the initial settings snapshot once. */
export const settingsReady: Promise<void> = (async () => {
  await listen<Settings>(TAURI_EVENT.SETTINGS_UPDATED, (event) => {
    Object.assign(settingsState, event.payload);
  });

  const initial = await getSettings();

  Object.assign(settingsState, initial);
})();

/** Submit a settings patch and wait for the Rust result. */
export async function updateSettings(patch: SettingsPatch): Promise<Settings> {
  return invokeUpdateSettings(patch);
}

/** Restore all settings to their defaults. */
export async function resetSettings(): Promise<Settings> {
  return invokeResetSettings();
}
