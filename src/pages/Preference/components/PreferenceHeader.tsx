import { Input } from "antd";
import type { ChangeEvent, FC } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/utils/cn";
import { PREFERENCE_TAB_META, resolveSectionAccent } from "../constants";
import type { PreferenceSection, PreferenceTab } from "../types/preferences";
import {
  translatePreferenceSection,
  translatePreferenceTab,
} from "../utils/preferenceI18n";
import type { PreferenceSearchResult } from "../utils/preferenceSearch";
import PreferenceCountTag from "./PreferenceCountTag";
import PreferenceSearchResults from "./PreferenceSearchResults";

interface PreferenceHeaderProps {
  activeSectionId: string;
  activeTab: PreferenceTab;
  searchQuery: string;
  searchResults: PreferenceSearchResult[];
  shouldReduceMotion: boolean;
  totalSettings: number;
  onPickSearchResult: (result: PreferenceSearchResult) => void;
  onSearchChange: (event: ChangeEvent<HTMLInputElement>) => void;
  onSectionSelect: (sectionId: string) => void;
}

/** Preferences header with title, global search, and section navigation. */
const PreferenceHeader: FC<PreferenceHeaderProps> = (props) => {
  const { t } = useTranslation(["preferences", "common"]);
  const {
    activeSectionId,
    activeTab,
    searchQuery,
    searchResults,
    shouldReduceMotion,
    totalSettings,
    onPickSearchResult,
    onSearchChange,
    onSectionSelect,
  } = props;

  return (
    <header
      className="shrink-0 border-ant-border-secondary border-b bg-ant-container px-6 pt-4 pb-2"
      data-tauri-drag-region
    >
      <div
        className="flex items-center justify-between gap-5"
        data-tauri-drag-region
      >
        <div className="min-w-0">
          <h1 className="m-0 flex items-center gap-2 font-semibold text-ant-text text-lg leading-snug">
            <i
              aria-hidden="true"
              className={cn(
                "text-lg",
                PREFERENCE_TAB_META[activeTab.id].accent.icon,
                PREFERENCE_TAB_META[activeTab.id].icon,
              )}
            />
            <span className="truncate">
              {translatePreferenceTab(t, activeTab)}
            </span>
          </h1>
        </div>

        <div className="flex shrink-0 items-center">
          <div className="relative z-3 w-64">
            <Input
              allowClear
              autoCapitalize="off"
              autoCorrect="off"
              className="border-ant-border-secondary bg-ant-fill-quaternary text-ant-text"
              onChange={onSearchChange}
              placeholder={t("preferences:search.placeholder")}
              prefix={
                <i
                  aria-hidden="true"
                  className="i-lucide:search text-ant-secondary text-base"
                />
              }
              spellCheck={false}
              value={searchQuery}
            />

            <PreferenceSearchResults
              onPick={onPickSearchResult}
              query={searchQuery.trim()}
              results={searchResults}
              shouldReduceMotion={shouldReduceMotion}
            />
          </div>
        </div>
      </div>

      <SectionTabs
        activeSectionId={activeSectionId}
        onSectionSelect={onSectionSelect}
        sections={activeTab.sections}
        totalSettings={totalSettings}
      />
    </header>
  );
};

interface SectionTabsProps {
  activeSectionId: string;
  sections: PreferenceSection[];
  totalSettings: number;
  onSectionSelect: (sectionId: string) => void;
}

/** Secondary preferences navigation beneath the title bar. */
const SectionTabs: FC<SectionTabsProps> = (props) => {
  const { t } = useTranslation(["preferences", "common"]);
  const { activeSectionId, sections, totalSettings, onSectionSelect } = props;

  return (
    <div className="mt-3 flex items-center gap-3" data-tauri-drag-region>
      {sections.map((section) => {
        const selected = section.id === activeSectionId;
        const handleClick = () => {
          onSectionSelect(section.id);
        };

        return (
          <button
            className={cn(
              "relative h-8 cursor-pointer whitespace-nowrap rounded-full border-0 px-3 font-medium text-sm transition-colors focus-visible:ring-1 focus-visible:ring-ant-primary motion-reduce:transition-none",
              selected
                ? resolveSectionAccent(section.id).pill
                : "bg-transparent text-ant-secondary hover:bg-ant-fill-tertiary hover:text-ant-text",
            )}
            key={section.id}
            onClick={handleClick}
            type="button"
          >
            {translatePreferenceSection(t, section, "title")}
          </button>
        );
      })}

      <PreferenceCountTag className="ml-auto">
        {t("common:units.settings", { count: totalSettings })}
      </PreferenceCountTag>
    </div>
  );
};

export default PreferenceHeader;
