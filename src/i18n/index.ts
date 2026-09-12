import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import clipboardEnUS from "@/locales/en-US/clipboard.json";
import commandsEnUS from "@/locales/en-US/commands.json";
import commonEnUS from "@/locales/en-US/common.json";
import onboardingEnUS from "@/locales/en-US/onboarding.json";
import preferencesEnUS from "@/locales/en-US/preferences.json";
import previewEnUS from "@/locales/en-US/preview.json";
import updateEnUS from "@/locales/en-US/update.json";
import clipboardKoKR from "@/locales/ko-KR/clipboard.json";
import commandsKoKR from "@/locales/ko-KR/commands.json";
import commonKoKR from "@/locales/ko-KR/common.json";
import onboardingKoKR from "@/locales/ko-KR/onboarding.json";
import preferencesKoKR from "@/locales/ko-KR/preferences.json";
import previewKoKR from "@/locales/ko-KR/preview.json";
import updateKoKR from "@/locales/ko-KR/update.json";
import clipboardZhCN from "@/locales/zh-CN/clipboard.json";
import commandsZhCN from "@/locales/zh-CN/commands.json";
import commonZhCN from "@/locales/zh-CN/common.json";
import onboardingZhCN from "@/locales/zh-CN/onboarding.json";
import preferencesZhCN from "@/locales/zh-CN/preferences.json";
import previewZhCN from "@/locales/zh-CN/preview.json";
import updateZhCN from "@/locales/zh-CN/update.json";
import type { Language } from "@/types/settings";

export const DEFAULT_LANGUAGE: Language = "ko-KR";
export const I18N_NAMESPACES = [
  "common",
  "commands",
  "clipboard",
  "onboarding",
  "preferences",
  "preview",
  "update",
] as const;

/**
 * react-i18next 初始化入口；资源来自 JSON，key 使用稳定语义路径。
 */
void i18n.use(initReactI18next).init({
  defaultNS: "common",
  fallbackLng: DEFAULT_LANGUAGE,
  interpolation: {
    escapeValue: false,
  },
  lng: DEFAULT_LANGUAGE,
  ns: I18N_NAMESPACES,
  resources: {
    "en-US": {
      clipboard: clipboardEnUS,
      commands: commandsEnUS,
      common: commonEnUS,
      onboarding: onboardingEnUS,
      preferences: preferencesEnUS,
      preview: previewEnUS,
      update: updateEnUS,
    },
    "ko-KR": {
      clipboard: clipboardKoKR,
      commands: commandsKoKR,
      common: commonKoKR,
      onboarding: onboardingKoKR,
      preferences: preferencesKoKR,
      preview: previewKoKR,
      update: updateKoKR,
    },
    "zh-CN": {
      clipboard: clipboardZhCN,
      commands: commandsZhCN,
      common: commonZhCN,
      onboarding: onboardingZhCN,
      preferences: preferencesZhCN,
      preview: previewZhCN,
      update: updateZhCN,
    },
  },
});

export default i18n;
