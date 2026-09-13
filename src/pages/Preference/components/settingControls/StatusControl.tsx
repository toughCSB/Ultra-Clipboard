import { Switch, Tag } from "antd";
import type { FC } from "react";
import { useTranslation } from "react-i18next";
import type { PreferenceSetting } from "../../types/preferences";
import { translatePreferenceControlLabel } from "../../utils/preferenceI18n";
import ControlFrame from "./ControlFrame";

interface StatusControlProps {
  setting: PreferenceSetting;
}

/** Display non-configurable capability status with switches or tags. */
const StatusControl: FC<StatusControlProps> = (props) => {
  const { t } = useTranslation("preferences");
  const { setting } = props;

  if (setting.status === "alwaysOn") {
    return (
      <ControlFrame>
        <Switch checked disabled />
      </ControlFrame>
    );
  }

  return (
    <ControlFrame>
      <Tag>{translatePreferenceControlLabel(t, setting)}</Tag>
    </ControlFrame>
  );
};

export default StatusControl;
