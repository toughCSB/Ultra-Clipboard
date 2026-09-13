import type { Event, UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useMount, useUnmount } from "ahooks";
import { type ThemeConfig, theme } from "antd";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Theme as SettingsTheme } from "@/types/settings";
import { log } from "@/utils/log";

type ResolvedTheme = "light" | "dark";
type NativeTheme = ResolvedTheme | null;

const resolveTheme = (mode: SettingsTheme, systemTheme: ResolvedTheme) => {
  if (mode === "dark") return "dark";
  if (mode === "light") return "light";

  return systemTheme;
};

const resolveNativeTheme = (mode: SettingsTheme): NativeTheme => {
  if (mode === "auto") return null;

  return mode;
};

const normalizeTauriTheme = (value: ResolvedTheme | null): ResolvedTheme => {
  if (value === "dark") return "dark";

  return "light";
};

const syncTauriWindowTheme = async (
  mode: SettingsTheme,
): Promise<ResolvedTheme | null> => {
  const currentWindow = getCurrentWebviewWindow();
  const nativeTheme = resolveNativeTheme(mode);

  await currentWindow.setTheme(nativeTheme);

  if (nativeTheme !== null) return null;

  return normalizeTauriTheme(await currentWindow.theme());
};

export const useAppTheme = (mode: SettingsTheme): ThemeConfig => {
  const themeUnlistenRef = useRef<UnlistenFn | null>(null);
  const themeMountedRef = useRef(false);
  const [systemTheme, setSystemTheme] = useState<ResolvedTheme>("light");
  const resolvedTheme = resolveTheme(mode, systemTheme);
  const algorithm =
    resolvedTheme === "dark" ? theme.darkAlgorithm : theme.defaultAlgorithm;
  const antdTheme = useMemo(() => {
    return {
      algorithm,
      token: {
        borderRadius: 10,
        colorBgContainer: resolvedTheme === "dark" ? "#1a1b1e" : "#ffffff",
        colorPrimary: "#0f766e",
        fontFamily:
          'Pretendard, "Apple SD Gothic Neo", "Malgun Gothic", system-ui, sans-serif',
      },
    };
  }, [algorithm, resolvedTheme]);

  const handleTauriThemeChanged = (event: Event<ResolvedTheme>) => {
    setSystemTheme(event.payload);
  };

  const initializeTauriThemeListener = async () => {
    try {
      const currentWindow = getCurrentWebviewWindow();
      const currentTheme = await currentWindow.theme();
      const unlisten = await currentWindow.onThemeChanged(
        handleTauriThemeChanged,
      );

      setSystemTheme(normalizeTauriTheme(currentTheme));

      if (!themeMountedRef.current) {
        unlisten();
        return;
      }

      themeUnlistenRef.current = unlisten;
    } catch (error) {
      log.error("tauri theme listener failed", error);
    }
  };

  const cleanupTauriThemeListener = () => {
    themeMountedRef.current = false;

    if (!themeUnlistenRef.current) return;

    themeUnlistenRef.current();
    themeUnlistenRef.current = null;
  };

  useMount(() => {
    themeMountedRef.current = true;
    void initializeTauriThemeListener();
  });

  useUnmount(cleanupTauriThemeListener);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", resolvedTheme === "dark");
    root.classList.toggle("light", resolvedTheme === "light");
  }, [resolvedTheme]);

  useEffect(() => {
    let stale = false;

    const syncNativeTheme = async () => {
      try {
        const currentTheme = await syncTauriWindowTheme(mode);

        if (stale || currentTheme === null) return;

        setSystemTheme(currentTheme);
      } catch (error) {
        log.error("tauri native theme sync failed", error);
      }
    };

    void syncNativeTheme();

    return () => {
      stale = true;
    };
  }, [mode]);

  return antdTheme;
};
