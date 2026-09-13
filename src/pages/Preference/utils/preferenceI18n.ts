import type { TFunction } from "i18next";
import { GITHUB_URL } from "@/constants/urls";
import { isMac, isWin } from "@/utils/is";
import type {
  PreferenceOption,
  PreferenceSection,
  PreferenceSetting,
  PreferenceTab,
} from "../types/preferences";

type PreferenceTranslator = TFunction<"preferences">;

export function translatePreferenceTab(
  t: PreferenceTranslator,
  tab: PreferenceTab,
) {
  return t(`schema.tabs.${tab.id}.title`);
}

export function translatePreferenceSection(
  t: PreferenceTranslator,
  section: PreferenceSection,
  field: "description" | "title",
) {
  return t(`schema.sections.${section.id}.${field}`);
}

export function translatePreferenceSetting(
  t: PreferenceTranslator,
  setting: PreferenceSetting,
  field: "description" | "title",
) {
  if (setting.id === "about.github" && field === "description") {
    return GITHUB_URL;
  }

  const platformField = resolvePlatformPreferenceField(setting, field);
  if (platformField) {
    return t(platformField);
  }

  return t(`schema.settings.${setting.id}.${field}`);
}

function resolvePlatformPreferenceField(
  setting: PreferenceSetting,
  field: "description" | "title",
) {
  const platform = isMac ? "macos" : isWin ? "windows" : null;
  if (!platform) return null;

  if (
    setting.id !== "control.autoStart" &&
    setting.id !== "control.trayIcon" &&
    setting.id !== "control.dockIcon"
  ) {
    return null;
  }

  return `schema.settings.${setting.id}.${platform}.${field}`;
}

export function translatePreferenceOption(
  t: PreferenceTranslator,
  setting: PreferenceSetting,
  option: PreferenceOption,
) {
  return {
    ...option,
    label: t(`schema.settings.${setting.id}.options.${option.value}`),
  };
}

export function translatePreferenceControlLabel(
  t: PreferenceTranslator,
  setting: PreferenceSetting,
) {
  if (
    setting.control.type !== "action" &&
    setting.control.type !== "status" &&
    setting.control.type !== "sortableTree" &&
    setting.control.type !== "sortableCheckboxTree"
  ) {
    return "";
  }

  return t(`schema.settings.${setting.id}.controlLabel`);
}

export function translatePreferencePlaceholder(
  t: PreferenceTranslator,
  setting: PreferenceSetting,
) {
  if (
    setting.control.type !== "text" &&
    setting.control.type !== "textarea" &&
    setting.control.type !== "shortcutRecorder"
  ) {
    return "";
  }

  return t(`schema.settings.${setting.id}.placeholder`);
}

export function translatePreferenceNumberSuffix(
  t: PreferenceTranslator,
  setting: PreferenceSetting,
) {
  if (setting.control.type !== "number") return "";

  if (setting.control.suffixKey) {
    return t(`schema.numberSuffixes.${setting.control.suffixKey}`);
  }

  return "";
}

export function translatePreferenceShortcutLabel(
  t: PreferenceTranslator,
  setting: PreferenceSetting,
  shortcutIndex: number,
) {
  if (setting.control.type !== "shortcutTags") return "";

  return t(`schema.settings.${setting.id}.shortcuts.${shortcutIndex}.label`);
}
