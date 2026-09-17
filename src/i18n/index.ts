import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import clipboardEnUS from "@/locales/en-US/clipboard.json";
import commandsEnUS from "@/locales/en-US/commands.json";
import commonEnUS from "@/locales/en-US/common.json";
import onboardingEnUS from "@/locales/en-US/onboarding.json";
import preferencesEnUS from "@/locales/en-US/preferences.json";
import previewEnUS from "@/locales/en-US/preview.json";
import screenshotEnUS from "@/locales/en-US/screenshot.json";
import updateEnUS from "@/locales/en-US/update.json";
import clipboardKoKR from "@/locales/ko-KR/clipboard.json";
import commandsKoKR from "@/locales/ko-KR/commands.json";
import commonKoKR from "@/locales/ko-KR/common.json";
import onboardingKoKR from "@/locales/ko-KR/onboarding.json";
import preferencesKoKR from "@/locales/ko-KR/preferences.json";
import previewKoKR from "@/locales/ko-KR/preview.json";
import screenshotKoKR from "@/locales/ko-KR/screenshot.json";
import updateKoKR from "@/locales/ko-KR/update.json";
import type { Language } from "@/types/settings";

export const DEFAULT_LANGUAGE: Language = "ko-KR";
export const I18N_NAMESPACES = [
  "common",
  "commands",
  "clipboard",
  "onboarding",
  "preferences",
  "preview",
  "screenshot",
  "update",
] as const;

/** Initializes react-i18next with JSON resources and stable semantic keys. */
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
      screenshot: screenshotEnUS,
      update: updateEnUS,
    },
    "ko-KR": {
      clipboard: clipboardKoKR,
      commands: commandsKoKR,
      common: commonKoKR,
      onboarding: onboardingKoKR,
      preferences: preferencesKoKR,
      preview: previewKoKR,
      screenshot: screenshotKoKR,
      update: updateKoKR,
    },
  },
});

export default i18n;
