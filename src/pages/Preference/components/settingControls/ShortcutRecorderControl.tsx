import type { TFunction } from "i18next";
import type { FC } from "react";
import { useTranslation } from "react-i18next";
import ShortcutRecorder, {
  type ShortcutRecorderConflict,
} from "@/components/ShortcutRecorder";
import type { Settings } from "@/types/settings";
import type { PreferenceSetting } from "../../types/preferences";
import { translatePreferencePlaceholder } from "../../utils/preferenceI18n";
import type { ControlProps } from "./types";

interface ShortcutRecorderControlProps extends ControlProps {
  setting: PreferenceSetting;
  settings: Settings;
  value: string;
}

/** Record a global shortcut and persist it through preferences. */
const ShortcutRecorderControl: FC<ShortcutRecorderControlProps> = (props) => {
  const { t } = useTranslation("preferences");
  const { disabled, onChange, setting, settings, value } = props;

  if (setting.control.type !== "shortcutRecorder") return null;

  const conflicts = resolveGlobalShortcutConflicts(t, setting, settings);

  const handleChange = async (nextValue: string) => {
    await onChange(setting, nextValue);
  };

  return (
    <ShortcutRecorder
      conflicts={conflicts}
      disabled={disabled}
      onChange={handleChange}
      placeholder={translatePreferencePlaceholder(t, setting)}
      value={value}
    />
  );
};

export default ShortcutRecorderControl;

/** Ensure every global shortcut remains mutually exclusive. */
function resolveGlobalShortcutConflicts(
  t: TFunction<"preferences">,
  setting: PreferenceSetting,
  settings: Settings,
) {
  const globalShortcuts: Record<string, string> = {
    "shortcuts.captureArea": settings.shortcuts.captureArea,
    "shortcuts.captureDelayed": settings.shortcuts.captureDelayed,
    "shortcuts.captureFullscreen": settings.shortcuts.captureFullscreen,
    "shortcuts.captureRepeat": settings.shortcuts.captureRepeat,
    "shortcuts.captureWindow": settings.shortcuts.captureWindow,
    "shortcuts.openClipboard": settings.shortcuts.openClipboard,
    "shortcuts.openPreference": settings.shortcuts.openPreference,
  };

  if (!(setting.id in globalShortcuts)) {
    return [] satisfies ShortcutRecorderConflict[];
  }

  return Object.entries(globalShortcuts)
    .filter(([id]) => {
      return id !== setting.id;
    })
    .map(([id, value]) => {
      return { label: translateShortcutSettingTitle(t, id), value };
    }) satisfies ShortcutRecorderConflict[];
}

/** Get the setting title used to identify a conflicting shortcut. */
function translateShortcutSettingTitle(
  t: TFunction<"preferences">,
  id: string,
) {
  return t(`schema.settings.${id}.title`);
}
