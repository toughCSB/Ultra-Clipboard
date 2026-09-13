import { setAutostart, showTaskbarIcon } from "@/commands";
import { updateSettings } from "@/stores/settings";
import type { SettingsPatch } from "@/types/settings";
import type {
  PreferenceSetting,
  SettingValue,
  SortableCheckboxTreeSettingValue,
} from "../types/preferences";

export async function commitSettingChange(
  setting: PreferenceSetting,
  value: SettingValue,
) {
  if (!setting.path) return;

  await applySideEffects(setting, value);
  await updateSettings(buildSettingPatch(setting, value));
}

export function settingValuesEqual(left: SettingValue, right: SettingValue) {
  if (Array.isArray(left) || Array.isArray(right)) {
    if (!Array.isArray(left) || !Array.isArray(right)) return false;
    if (left.length !== right.length) return false;

    return left.every((item, index) => {
      return item === right[index];
    });
  }

  if (typeof left === "object" || typeof right === "object") {
    if (typeof left !== "object" || typeof right !== "object") return false;
    if (left === null || right === null) return false;

    return JSON.stringify(left) === JSON.stringify(right);
  }

  return left === right;
}

function buildPatch(
  path: readonly string[],
  value: SettingValue,
): SettingsPatch {
  const [head, ...rest] = path;
  if (!head) return {};

  return {
    [head]: buildNestedPatch(rest, value),
  } as SettingsPatch;
}

function buildSettingPatch(
  setting: PreferenceSetting,
  value: SettingValue,
): SettingsPatch {
  if (!setting.path) return {};

  if (
    setting.control.type === "sortableCheckboxTree" &&
    isSortableCheckboxTreeValue(value)
  ) {
    return mergeSettingsPatch(
      buildPatch(setting.path, value.selected),
      buildPatch(setting.control.orderPath, value.order),
    );
  }

  return buildPatch(setting.path, value);
}

function isSortableCheckboxTreeValue(
  value: SettingValue,
): value is SortableCheckboxTreeSettingValue {
  if (typeof value !== "object" || value === null) return false;
  if (!("selected" in value) || !("order" in value)) return false;

  return Array.isArray(value.selected) && Array.isArray(value.order);
}

function mergeSettingsPatch(
  left: SettingsPatch,
  right: SettingsPatch,
): SettingsPatch {
  return mergePatchValue(left, right) as SettingsPatch;
}

function mergePatchValue(left: unknown, right: unknown): unknown {
  if (!isPlainObject(left) || !isPlainObject(right)) return right;

  const merged: Record<string, unknown> = { ...left };

  for (const [key, value] of Object.entries(right)) {
    merged[key] = mergePatchValue(merged[key], value);
  }

  return merged;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function buildNestedPatch(
  path: readonly string[],
  value: SettingValue,
): SettingValue | Record<string, unknown> {
  const [head, ...rest] = path;
  if (!head) return value;

  return {
    [head]: buildNestedPatch(rest, value),
  };
}

async function applySideEffects(
  setting: PreferenceSetting,
  value: SettingValue,
) {
  if (setting.id === "control.autoStart") {
    await setAutostart(Boolean(value));
    return;
  }

  if (setting.id === "control.dockIcon") {
    await showTaskbarIcon(Boolean(value));
  }
}
