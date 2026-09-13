import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { platform } from "@tauri-apps/plugin-os";
import { WINDOW_LABEL } from "@/constants/windows";
/** Whether the app is running on macOS. */
export const isMac = platform() === "macos";
/** Whether the app is running on Windows. */
export const isWin = platform() === "windows";
/** Whether this is a Vite development build. */
export const isDev = import.meta.env.DEV;
/** Whether the current WebView is the non-focusable Windows clipboard window. */
export const isWinClipboardWindow = () => {
  return isWin && getCurrentWebviewWindow().label === WINDOW_LABEL.CLIPBOARD;
};
/** Whether a path or filename has a common image extension. */
export const isImage = (value: string) => {
  const regex = /\.(jpe?g|png|webp|avif|gif|svg|bmp|ico|tiff?|heic|apng)$/i;

  return regex.test(value);
};
