import { allPreferenceSettings } from "../config/preferenceSchema";
import {
  translatePreferenceSection,
  translatePreferenceSetting,
  translatePreferenceTab,
} from "./preferenceI18n";

export type PreferenceSearchResult = (typeof allPreferenceSettings)[number];
export type PreferenceSearchTranslator = Parameters<
  typeof translatePreferenceTab
>[0];

export function searchPreferenceSettings(
  query: string,
  t: PreferenceSearchTranslator,
): PreferenceSearchResult[] {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return [];

  return allPreferenceSettings
    .filter(({ section, setting, tab }) => {
      const haystack = [
        translatePreferenceTab(t, tab),
        translatePreferenceSection(t, section, "title"),
        translatePreferenceSetting(t, setting, "title"),
        translatePreferenceSetting(t, setting, "description"),
        ...(setting.keywords ?? []),
      ]
        .join(" ")
        .toLowerCase();

      return haystack.includes(normalized);
    })
    .slice(0, 8);
}
