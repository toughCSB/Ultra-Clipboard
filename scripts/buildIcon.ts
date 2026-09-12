import { execSync } from "node:child_process";

const IOS_BACKGROUND_COLOR = "#071534";

(() => {
  const { env, platform } = process;

  const isMac = env.PLATFORM?.startsWith("macos") ?? platform === "darwin";

  const logoName = isMac ? "logo-mac" : "logo";
  const tauriCommand =
    platform === "win32"
      ? "node_modules/.bin/tauri.cmd"
      : "node_modules/.bin/tauri";

  const command = `"${tauriCommand}" icon src-tauri/assets/${logoName}.svg --ios-color "${IOS_BACKGROUND_COLOR}"`;

  execSync(command, { stdio: "inherit" });
})();
