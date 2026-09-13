import { theme as antdTheme } from "antd";
import type { Preset } from "unocss";
/** UnoCSS preset exposing Ant Design color tokens under the `ant-*` prefix. */
export interface PresetAntdColorsOptions {
  /** CSS variable prefix, matching Ant Design's `getPrefixCls()` prefix. */
  antPrefix?: string;
}

type ColorMap = Record<string, string>;

const PALETTE_RE = /^([a-z]+)-(\d{1,2})$/;
const COLOR_PREFIX_ALIASES = ["bg-", "text-", "border-", "fill-"] as const;
/** Convert an Ant Design token key to kebab case. */
const kebab = (value: string): string => {
  return value.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();
};
/** Collect Ant Design color tokens into a flat `theme.colors` map. */
const collectAntdColors = (antPrefix: string): ColorMap => {
  const colors: ColorMap = {};

  for (const key of Object.keys(antdTheme.getDesignToken())) {
    if (/^color[A-Z]/.test(key)) {
      const short = kebab(key.slice("color".length));
      const cssVariable = `var(--${antPrefix}-color-${short})`;
      colors[`ant-${short}`] = cssVariable;

      for (const prefix of COLOR_PREFIX_ALIASES) {
        if (!short.startsWith(prefix)) continue;

        colors[`ant-${short.slice(prefix.length)}`] ??= cssVariable;
      }
      continue;
    }

    const paletteMatch = PALETTE_RE.exec(key);
    if (!paletteMatch) continue;

    const [, palette, step] = paletteMatch;
    colors[`ant-${palette}-${step}`] = `var(--${antPrefix}-${key})`;
  }

  return colors;
};
/** Create the Ant Design color preset. */
export const presetAntdColors = (
  options: PresetAntdColorsOptions = {},
): Preset => {
  const antPrefix = options.antPrefix ?? "ant";
  const colors = collectAntdColors(antPrefix);

  return {
    extendTheme: (theme: { colors?: ColorMap }) => {
      theme.colors = {
        ...theme.colors,
        ...colors,
      };
    },
    name: "preset-antd-colors",
  };
};

export default presetAntdColors;
