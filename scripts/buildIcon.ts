import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { formatTrayIconReport, validateTrayIcon } from "./validateTrayIcon.ts";

const IOS_BACKGROUND_COLOR = "#071534";
const TAURI_CLI_PATH = resolve("node_modules/@tauri-apps/cli/tauri.js");
const WINDOWS_TRAY_SOURCE_PATH = resolve("src-tauri/assets/tray-windows.png");
const WINDOWS_TRAY_ICON_PATH = resolve("src-tauri/assets/tray.ico");

const runTauriIcon = (sourcePath: string, outputPath?: string) => {
  const args = [
    TAURI_CLI_PATH,
    "icon",
    sourcePath,
    "--ios-color",
    IOS_BACKGROUND_COLOR,
  ];
  if (outputPath !== undefined) args.push("--output", outputPath);

  execFileSync(process.execPath, args, { stdio: "inherit" });
};

const generateWindowsTrayIcon = () => {
  const outputPath = mkdtempSync(join(tmpdir(), "ultra-clipboard-tray-icon-"));

  try {
    runTauriIcon(WINDOWS_TRAY_SOURCE_PATH, outputPath);
    const generatedIconPath = join(outputPath, "icon.ico");
    const report = validateTrayIcon(generatedIconPath);

    copyFileSync(generatedIconPath, WINDOWS_TRAY_ICON_PATH);
    process.stdout.write(`${formatTrayIconReport(report)}\n`);
  } finally {
    rmSync(outputPath, { force: true, recursive: true });
  }
};

(() => {
  const { env, platform } = process;

  const isMac = env.PLATFORM?.startsWith("macos") ?? platform === "darwin";
  const isWindows = env.PLATFORM?.startsWith("windows") ?? platform === "win32";

  const logoName = isMac ? "logo-mac" : "logo";
  const logoPath = resolve(`src-tauri/assets/${logoName}.png`);

  runTauriIcon(logoPath);
  if (isWindows) generateWindowsTrayIcon();
})();
