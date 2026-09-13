import { Space, Tag } from "antd";
import type { FC } from "react";
import { useTranslation } from "react-i18next";
import type { PreferenceSetting } from "../../types/preferences";
import { translatePreferenceShortcutLabel } from "../../utils/preferenceI18n";

interface ShortcutTagsControlProps {
  setting: PreferenceSetting;
}

/** Display a read-only shortcut; clipboard-window shortcuts are code-defined. */
const ShortcutTagsControl: FC<ShortcutTagsControlProps> = (props) => {
  const { setting } = props;
  const { t } = useTranslation("preferences");

  if (setting.control.type !== "shortcutTags") return null;

  return (
    <Space orientation="vertical">
      {setting.control.shortcuts.map((shortcut, shortcutIndex) => {
        const label = translatePreferenceShortcutLabel(
          t,
          setting,
          shortcutIndex,
        );

        return (
          <Space key={label}>
            {shortcut.keys.map((key) => {
              return <Tag key={`${label}-${key}`}>{key}</Tag>;
            })}
          </Space>
        );
      })}
    </Space>
  );
};

export default ShortcutTagsControl;
