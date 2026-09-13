import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEventListener, useMount } from "ahooks";
import type { ConfigProviderProps } from "antd";
import { App as AntdApp, ConfigProvider } from "antd";
import enUS from "antd/locale/en_US";
import koKR from "antd/locale/ko_KR";
import type { FC } from "react";
import { use, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { RouterProvider } from "react-router";
import { useSnapshot } from "valtio";
import { notifyWindowReady } from "@/commands";
import { WINDOW_LABEL } from "@/constants/windows";
import { useAppTheme } from "@/hooks/useAppTheme";
import { router } from "./router";
import { settingsReady, settingsState } from "./stores/settings";
import "./stores/windowLifecycle";
import type { Language } from "./types/settings";
import { setMessageApi, setModalApi } from "./utils/feedback";
import { log } from "./utils/log";

const ANTD_MODAL_CONFIG = {
  centered: true,
} satisfies ConfigProviderProps["modal"];

const resolveAntdLocale = (language: Language) => {
  if (language === "en-US") return enUS;

  return koKR;
};

const AppContent: FC = () => {
  const { message, modal } = AntdApp.useApp();

  useEffect(() => {
    setMessageApi(message);
    setModalApi(modal);
  }, [message, modal]);

  return <RouterProvider router={router} />;
};

const App: FC = () => {
  use(settingsReady);

  const { i18n } = useTranslation();
  const settings = useSnapshot(settingsState);
  const windowLabel = getCurrentWebviewWindow().label;
  const mode =
    windowLabel === WINDOW_LABEL.ONBOARDING
      ? "dark"
      : settings.appearance.theme;
  const language = settings.appearance.language;
  const antdTheme = useAppTheme(mode);
  const locale = resolveAntdLocale(language);

  useEffect(() => {
    document.documentElement.lang = language;

    if (i18n.language === language) return;

    void i18n.changeLanguage(language);
  }, [i18n, language]);

  useMount(async () => {
    await notifyWindowReady(getCurrentWebviewWindow().label);
  });

  useEventListener("unhandledrejection", (event) => {
    const { reason } = event;

    log.error(
      "unhandled promise rejection",
      reason instanceof Error ? reason : { reason },
    );
  });

  useEventListener("error", (event) => {
    const { error, ...rest } = event;

    log.error("uncaught error", error instanceof Error ? error : rest);
  });

  return (
    <ConfigProvider locale={locale} modal={ANTD_MODAL_CONFIG} theme={antdTheme}>
      <AntdApp>
        <AppContent />
      </AntdApp>
    </ConfigProvider>
  );
};

export default App;
