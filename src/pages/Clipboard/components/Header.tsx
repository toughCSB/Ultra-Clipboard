import { useDebounceFn } from "ahooks";
import type { MenuProps } from "antd";
import type { ChangeEvent, FC } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";
import {
  clearClipboardItems,
  setClipboardWindowPinned,
  showWindow,
} from "@/commands";
import CustomIconButton from "@/components/CustomIconButton";
import Dropdown, {
  type AppDropdownProps,
  type DropdownMenuItems,
} from "@/components/Dropdown";
import KeyHint from "@/components/KeyHint";
import Tooltip from "@/components/Tooltip";
import { TAURI_EVENT } from "@/constants/events";
import { WINDOW_LABEL } from "@/constants/windows";
import { useTauriListen } from "@/hooks/useTauriListen";
import { clipboardViewState } from "@/stores/clipboardView";
import { settingsState } from "@/stores/settings";
import { formatShortcutDisplay } from "@/utils/shortcut";
import SearchInput from "./SearchInput";

interface WindowVisibilityPayload {
  label: string;
  visible: boolean;
}

type HeaderMoreMenuKey = "clear" | "preference";

const MORE_ACTION_TRIGGER: AppDropdownProps["trigger"] = ["click"];
const PREFERENCE_SHORTCUT = formatShortcutDisplay("CmdOrCtrl+,", " ");
/** Clipboard window header with logo, search, pin, and more actions. */
const Header: FC = () => {
  const { t } = useTranslation("clipboard");
  const settings = useSnapshot(settingsState);
  const [pinned, setPinned] = useState(false);
  const [searchBlurToken, setSearchBlurToken] = useState(0);
  const [searchClearToken, setSearchClearToken] = useState(0);
  const [searchFocusToken, setSearchFocusToken] = useState(0);

  /** Open preferences from the button or keyboard shortcut. */
  const handleOpenPreference = () => {
    return showWindow(WINDOW_LABEL.PREFERENCE);
  };

  /** Clear clipboard history through the shared command wrapper. */
  const handleClearClipboardItems = async () => {
    await clearClipboardItems();
  };

  /** Dispatch menu actions, using the command wrapper for confirmation. */
  const handleMoreMenuClick: MenuProps["onClick"] = async (info) => {
    const key = info.key as HeaderMoreMenuKey;

    if (key === "clear") {
      await handleClearClipboardItems();

      return;
    }

    await handleOpenPreference();
  };

  /** Toggle the pinned state in Rust and update the button state. */
  const handleTogglePinned = async () => {
    const next = !pinned;

    await setClipboardWindowPinned(next);
    setPinned(next);
  };

  /** Debounced search updates keep the input uncontrolled during IME composition. */
  const { cancel: cancelKeywordChange, run: handleKeywordChange } =
    useDebounceFn(
      (event: ChangeEvent<HTMLInputElement>) => {
        clipboardViewState.keyword = event.target.value.trim();
      },
      { wait: 200 },
    );

  /** Clear the shared query and remount the input. */
  const clearSearch = () => {
    cancelKeywordChange();
    clipboardViewState.keyword = "";

    setSearchClearToken((current) => {
      return current + 1;
    });
  };

  /** Blur the search input when the window is hidden. */
  const blurSearch = () => {
    setSearchBlurToken((current) => {
      return current + 1;
    });
  };

  /** Focus the search input after the window becomes visible. */
  const focusSearch = () => {
    setSearchFocusToken((current) => {
      return current + 1;
    });
  };

  const handleWindowVisibility = (event: {
    payload: WindowVisibilityPayload;
  }) => {
    const { label, visible } = event.payload;
    if (label !== WINDOW_LABEL.CLIPBOARD) return;

    if (!visible) {
      blurSearch();

      if (settings.clipboard.search.clearOnHide) {
        clearSearch();
      }

      return;
    }

    if (settings.clipboard.search.clearOnHide) {
      clearSearch();
    }

    if (!settings.clipboard.search.defaultFocus) {
      blurSearch();

      return;
    }

    focusSearch();
  };

  useTauriListen<WindowVisibilityPayload>(
    TAURI_EVENT.WINDOW_VISIBILITY,
    handleWindowVisibility,
  );

  const moreMenuItems: DropdownMenuItems = [
    {
      extra: PREFERENCE_SHORTCUT,
      icon: "i-lucide:settings",
      key: "preference",
      label: t("header.openPreference"),
    },
    {
      danger: true,
      icon: "i-lucide:trash-2",
      key: "clear",
      label: t("header.clearRecords"),
    },
  ];

  return (
    <div
      className="flex items-center justify-between border-teal-400 border-b-2 bg-teal-50 p-3 pb-2 dark:border-teal-500 dark:bg-teal-950/40"
      data-tauri-drag-region
    >
      <img alt={t("header.logoAlt")} className="size-5" src="/logo.png" />

      <div className="flex items-center gap-1">
        <SearchInput
          allowClear
          blurToken={searchBlurToken}
          className="w-40"
          clearToken={searchClearToken}
          focusToken={searchFocusToken}
          onChange={handleKeywordChange}
          placeholder={t("header.searchPlaceholder")}
          size="small"
        />

        <Tooltip title={t(pinned ? "header.unpin" : "header.pin")}>
          <CustomIconButton
            icon={
              <KeyHint
                hintKey="P"
                iconName="i-lets-icons:pin"
                onKeyPress={handleTogglePinned}
              />
            }
            onClick={handleTogglePinned}
            size="small"
            type={pinned ? "primary" : "text"}
          />
        </Tooltip>

        <Dropdown
          menu={{ items: moreMenuItems, onClick: handleMoreMenuClick }}
          tooltip={t("header.moreActions")}
          trigger={MORE_ACTION_TRIGGER}
        >
          <CustomIconButton
            icon={
              <KeyHint
                hintKey=","
                iconName="i-lets-icons:meatballs-menu"
                onKeyPress={handleOpenPreference}
              />
            }
            size="small"
            type="text"
          />
        </Dropdown>
      </div>
    </div>
  );
};

export default Header;
