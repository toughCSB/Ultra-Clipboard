import { motion } from "motion/react";
import type { FC } from "react";
import { useTranslation } from "react-i18next";
import type {
  ChangeStorageLocationResult,
  CleanCacheResult,
  ExportHistoryBackupResult,
  StorageLocation,
} from "@/commands";
import type { Settings } from "@/types/settings";
import { cn } from "@/utils/cn";
import { resolveSectionAccent } from "../constants";
import type {
  PreferenceSection as PreferenceSectionModel,
  PreferenceSetting,
  SettingValue,
} from "../types/preferences";
import { translatePreferenceSection } from "../utils/preferenceI18n";
import PreferenceCountTag from "./PreferenceCountTag";
import PreferenceSettingRow from "./PreferenceSettingRow";
import SourceAppsTransfer from "./SourceAppsTransfer";

interface PreferenceSectionProps {
  highlightedSettingId: string | null;
  highlightToken: number;
  section: PreferenceSectionModel;
  settings: Settings;
  shouldReduceMotion: boolean;
  storageLocation: StorageLocation | null;
  onActionComplete?: (
    setting: PreferenceSetting,
    result?:
      | ChangeStorageLocationResult
      | CleanCacheResult
      | ExportHistoryBackupResult,
  ) => void;
  onChange: (setting: PreferenceSetting, value: SettingValue) => Promise<void>;
}

interface SectionVisual {
  icon: string;
}

/** Semantic group in the main preferences content. */
const PreferenceSection: FC<PreferenceSectionProps> = (props) => {
  const { t } = useTranslation(["preferences", "common"]);
  const {
    highlightedSettingId,
    highlightToken,
    section,
    settings,
    shouldReduceMotion,
    storageLocation,
    onActionComplete,
    onChange,
  } = props;
  const visual = resolveSectionVisual(section.id);
  const accent = resolveSectionAccent(section.id);
  const sourceAppsSettings = resolveSourceAppsSettings(section.settings);

  if (sourceAppsSettings) {
    return (
      <motion.section
        animate={{ opacity: 1 }}
        className="relative flex min-h-0 flex-1 scroll-mt-5 flex-col overflow-hidden rounded-3 border border-ant-border-secondary bg-ant-container"
        id={section.id}
        initial={{ opacity: 0 }}
        transition={{
          duration: shouldReduceMotion ? 0 : 0.12,
          ease: "easeOut",
        }}
      >
        <span
          className={cn("absolute inset-y-0 left-0 w-1.5", accent.stripe)}
        />
        <div className="min-h-0 flex-1 p-4 pl-5">
          <SourceAppsTransfer
            excludedAppsSetting={sourceAppsSettings.excludedApps}
            onChange={onChange}
            settings={settings}
          />
        </div>
      </motion.section>
    );
  }

  return (
    <motion.section
      animate={{ opacity: 1 }}
      className="relative scroll-mt-5 overflow-hidden rounded-3 border border-ant-border-secondary bg-ant-container"
      id={section.id}
      initial={{ opacity: 0 }}
      transition={{
        duration: shouldReduceMotion ? 0 : 0.12,
        ease: "easeOut",
      }}
    >
      <span className={cn("absolute inset-y-0 left-0 w-1", accent.stripe)} />
      <div
        className={cn(
          "relative flex items-center justify-between gap-4 border-ant-split border-b px-5 py-3.5",
          accent.wash,
        )}
      >
        <div className="flex min-w-0 items-center gap-3">
          <span
            className={cn(
              "flex size-9 shrink-0 items-center justify-center rounded-2 text-xl",
              accent.chip,
            )}
          >
            <i aria-hidden="true" className={visual.icon} />
          </span>

          <div className="min-w-0">
            <h2 className="m-0 truncate font-semibold text-ant-text text-sm leading-tight">
              {translatePreferenceSection(t, section, "title")}
            </h2>
          </div>
        </div>

        <PreferenceCountTag>
          {t("common:units.items", { count: section.settings.length })}
        </PreferenceCountTag>
      </div>

      <div>
        {section.settings.map((setting) => {
          return (
            <PreferenceSettingRow
              highlighted={setting.id === highlightedSettingId}
              highlightToken={highlightToken}
              key={setting.id}
              onActionComplete={onActionComplete}
              onChange={onChange}
              setting={setting}
              settings={settings}
              shouldReduceMotion={shouldReduceMotion}
              storageLocation={storageLocation}
            />
          );
        })}
      </div>
    </motion.section>
  );
};

export default PreferenceSection;

/** Choose a small icon that matches the section meaning. */
function resolveSectionVisual(id: string): SectionVisual {
  const normalizedId = id.toLowerCase();

  if (normalizedId.includes("about")) {
    return {
      icon: "i-lucide:info",
    };
  }

  if (normalizedId.includes("capture")) {
    return {
      icon: "i-lucide:clipboard-plus",
    };
  }

  if (normalizedId.includes("source")) {
    return {
      icon: "i-lucide:panels-top-left",
    };
  }

  if (
    normalizedId.includes("sensitive") ||
    normalizedId.includes("diagnostics") ||
    normalizedId.includes("permissions")
  ) {
    return {
      icon: "i-lucide:shield-check",
    };
  }

  if (normalizedId.includes("history") || normalizedId.includes("localdata")) {
    return {
      icon: "i-lucide:database",
    };
  }

  if (normalizedId.includes("organizing")) {
    return {
      icon: "i-lucide:star",
    };
  }

  if (normalizedId.includes("groups")) {
    return {
      icon: "i-lucide:folder-tree",
    };
  }

  if (normalizedId.includes("search")) {
    return {
      icon: "i-lucide:search",
    };
  }

  if (normalizedId.includes("shortcuts")) {
    return {
      icon: "i-lucide:keyboard",
    };
  }

  if (normalizedId.includes("paste") || normalizedId.includes("actions")) {
    return {
      icon: "i-lucide:mouse-pointer-click",
    };
  }

  if (normalizedId.includes("copy")) {
    return {
      icon: "i-lucide:copy",
    };
  }

  if (normalizedId.includes("window")) {
    return {
      icon: "i-lucide:panel-left",
    };
  }

  if (normalizedId.includes("preview")) {
    return {
      icon: "i-lucide:eye",
    };
  }

  if (normalizedId.includes("appearance")) {
    return {
      icon: "i-lucide:paintbrush",
    };
  }

  if (normalizedId.includes("control")) {
    return {
      icon: "i-lucide:monitor",
    };
  }

  if (normalizedId.includes("webdav")) {
    return {
      icon: "i-lucide:cloud",
    };
  }

  if (normalizedId.includes("backup")) {
    return {
      icon: "i-lucide:refresh-cw",
    };
  }

  if (normalizedId.includes("updates")) {
    return {
      icon: "i-lucide:refresh-cw",
    };
  }

  return {
    icon: "i-lucide:clipboard-list",
  };
}

function resolveSourceAppsSettings(settings: PreferenceSetting[]) {
  const excludedApps = settings.find((setting) => {
    return setting.id === "source.excludedApps";
  });

  if (!excludedApps) return null;

  return { excludedApps };
}
