import { Select } from "antd";
import type { FC } from "react";
import { useTranslation } from "react-i18next";
import type { PreferenceSetting, SettingValue } from "../../types/preferences";
import { translatePreferenceOption } from "../../utils/preferenceI18n";
import type { ControlProps } from "./types";

interface SegmentedSelectControlProps extends ControlProps {
  setting: PreferenceSetting;
  value: string;
}

/** Save short option values immediately using a segmented control. */
export const SegmentedSelectControl: FC<SegmentedSelectControlProps> = (
  props,
) => {
  const { t } = useTranslation("preferences");
  const { disabled, onChange, setting, value } = props;

  if (setting.control.type !== "segmented") return null;

  const options = setting.control.options.map((option) => {
    return translatePreferenceOption(t, setting, option);
  });

  const handleChange = async (next: string | number) => {
    await onChange(setting, next);
  };

  return (
    <Select
      disabled={disabled}
      onChange={handleChange}
      options={options}
      value={value}
    />
  );
};

interface SelectControlProps extends ControlProps {
  setting: PreferenceSetting;
  value?: SettingValue;
}

/** Save long or multi-select option values immediately. */
export const SelectControl: FC<SelectControlProps> = (props) => {
  const { t } = useTranslation("preferences");
  const { disabled, onChange, setting, value } = props;

  if (setting.control.type !== "select") return null;

  const options = setting.control.options.map((option) => {
    return translatePreferenceOption(t, setting, option);
  });

  const handleChange = async (next: string | number | string[] | number[]) => {
    await onChange(setting, next as SettingValue);
  };

  return (
    <Select
      disabled={disabled}
      mode={setting.control.mode}
      onChange={handleChange}
      options={options}
      value={value as string | number | string[] | number[]}
    />
  );
};
