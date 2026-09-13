import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";
import CustomIconButton from "@/components/CustomIconButton";
import KeyHint from "@/components/KeyHint";
import Popover from "@/components/Popover";
import { clipboardStatsState } from "@/stores/clipboardStats";
import ShortcutList from "./ShortcutList";

/** Footer bar showing the filtered item count and window shortcut hint. */
const Footer = () => {
  const { t } = useTranslation("clipboard");
  const { total } = useSnapshot(clipboardStatsState);

  const [popoverOpen, setPopoverOpen] = useState(false);

  const handleShortcutKeyPress = () => {
    setPopoverOpen((prev) => {
      return !prev;
    });
  };

  return (
    <div className="flex items-center justify-between px-3 py-1">
      <span className="text-ant-tertiary text-xs">
        {t("footer.total", { count: total ?? 0 })}
      </span>

      <Popover
        content={<ShortcutList />}
        onOpenChange={setPopoverOpen}
        open={popoverOpen}
        title={t("footer.shortcuts")}
        tooltip={t("footer.shortcuts")}
        trigger="click"
      >
        <CustomIconButton
          icon={
            <KeyHint
              hintKey="K"
              iconName="i-lucide:keyboard"
              onKeyPress={handleShortcutKeyPress}
            />
          }
          size="small"
          type="text"
        />
      </Popover>
    </div>
  );
};

export default Footer;
