import {
  defineConfig,
  presetIcons,
  presetWind4,
  transformerDirectives,
  transformerVariantGroup,
} from "unocss";
import { ALL_TOOLS } from "./src/pages/ScreenshotEditor/model/tools";
import { presetAntdColors } from "./src/unocss/presetAntdColors";

/**
 * Icon classes referenced only through a variable (an object built from an
 * array, then read dynamically) can be missed by the production build's
 * static class scan even though the string literal is right there in the
 * source. This screenshot editor's toolbar is entirely built from one such
 * array, so its icons are safelisted explicitly rather than relying on scan
 * order or file layout to keep working.
 */
const screenshotEditorIcons = ALL_TOOLS.map((tool) => tool.icon);

export default defineConfig({
  presets: [presetWind4(), presetAntdColors(), presetIcons()],
  safelist: screenshotEditorIcons,
  transformers: [
    transformerVariantGroup(),
    transformerDirectives({
      applyVariable: ["--uno"],
    }),
  ],
});
