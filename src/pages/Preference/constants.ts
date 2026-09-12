/* @unocss-include */
import type { PreferenceTabId } from "./types/preferences";

export const APP_NAME_PLACEHOLDER = "Ultra Clipboard";

export const STORAGE_WARNING_BYTES = 1024 * 1024 * 1024;
export const STORAGE_ERROR_BYTES = 2 * 1024 * 1024 * 1024;

export interface PreferenceAccent {
  chip: string;
  fill: string;
  icon: string;
  pill: string;
  stripe: string;
  wash: string;
}

interface PreferenceTabMeta {
  accent: PreferenceAccent;
  icon: string;
}

type AccentName =
  | "amber"
  | "blue"
  | "cyan"
  | "emerald"
  | "fuchsia"
  | "indigo"
  | "lime"
  | "orange"
  | "pink"
  | "red"
  | "rose"
  | "sky"
  | "slate"
  | "teal"
  | "violet"
  | "yellow";

const ACCENTS: Record<AccentName, PreferenceAccent> = {
  amber: {
    chip: "bg-amber-500/30 text-amber-600",
    fill: "bg-amber-500 text-white",
    icon: "text-amber-500",
    pill: "bg-amber-500 text-white",
    stripe: "bg-amber-500",
    wash: "bg-amber-500/30",
  },
  blue: {
    chip: "bg-blue-500/30 text-blue-600",
    fill: "bg-blue-500 text-white",
    icon: "text-blue-500",
    pill: "bg-blue-500 text-white",
    stripe: "bg-blue-500",
    wash: "bg-blue-500/30",
  },
  cyan: {
    chip: "bg-cyan-500/30 text-cyan-600",
    fill: "bg-cyan-500 text-white",
    icon: "text-cyan-500",
    pill: "bg-cyan-500 text-white",
    stripe: "bg-cyan-500",
    wash: "bg-cyan-500/30",
  },
  emerald: {
    chip: "bg-emerald-500/30 text-emerald-600",
    fill: "bg-emerald-500 text-white",
    icon: "text-emerald-500",
    pill: "bg-emerald-500 text-white",
    stripe: "bg-emerald-500",
    wash: "bg-emerald-500/30",
  },
  fuchsia: {
    chip: "bg-fuchsia-500/30 text-fuchsia-600",
    fill: "bg-fuchsia-500 text-white",
    icon: "text-fuchsia-500",
    pill: "bg-fuchsia-500 text-white",
    stripe: "bg-fuchsia-500",
    wash: "bg-fuchsia-500/30",
  },
  indigo: {
    chip: "bg-indigo-500/30 text-indigo-600",
    fill: "bg-indigo-500 text-white",
    icon: "text-indigo-500",
    pill: "bg-indigo-500 text-white",
    stripe: "bg-indigo-500",
    wash: "bg-indigo-500/30",
  },
  lime: {
    chip: "bg-lime-500/30 text-lime-700",
    fill: "bg-lime-500 text-white",
    icon: "text-lime-500",
    pill: "bg-lime-500 text-white",
    stripe: "bg-lime-500",
    wash: "bg-lime-500/30",
  },
  orange: {
    chip: "bg-orange-500/30 text-orange-600",
    fill: "bg-orange-500 text-white",
    icon: "text-orange-500",
    pill: "bg-orange-500 text-white",
    stripe: "bg-orange-500",
    wash: "bg-orange-500/30",
  },
  pink: {
    chip: "bg-pink-500/30 text-pink-600",
    fill: "bg-pink-500 text-white",
    icon: "text-pink-500",
    pill: "bg-pink-500 text-white",
    stripe: "bg-pink-500",
    wash: "bg-pink-500/30",
  },
  red: {
    chip: "bg-red-500/30 text-red-600",
    fill: "bg-red-500 text-white",
    icon: "text-red-500",
    pill: "bg-red-500 text-white",
    stripe: "bg-red-500",
    wash: "bg-red-500/30",
  },
  rose: {
    chip: "bg-rose-500/30 text-rose-600",
    fill: "bg-rose-500 text-white",
    icon: "text-rose-500",
    pill: "bg-rose-500 text-white",
    stripe: "bg-rose-500",
    wash: "bg-rose-500/30",
  },
  sky: {
    chip: "bg-sky-500/30 text-sky-600",
    fill: "bg-sky-500 text-white",
    icon: "text-sky-500",
    pill: "bg-sky-500 text-white",
    stripe: "bg-sky-500",
    wash: "bg-sky-500/30",
  },
  slate: {
    chip: "bg-slate-500/30 text-slate-600",
    fill: "bg-slate-500 text-white",
    icon: "text-slate-500",
    pill: "bg-slate-500 text-white",
    stripe: "bg-slate-500",
    wash: "bg-slate-500/30",
  },
  teal: {
    chip: "bg-teal-500/30 text-teal-600",
    fill: "bg-teal-500 text-white",
    icon: "text-teal-500",
    pill: "bg-teal-500 text-white",
    stripe: "bg-teal-500",
    wash: "bg-teal-500/30",
  },
  violet: {
    chip: "bg-violet-500/30 text-violet-600",
    fill: "bg-violet-500 text-white",
    icon: "text-violet-500",
    pill: "bg-violet-500 text-white",
    stripe: "bg-violet-500",
    wash: "bg-violet-500/30",
  },
  yellow: {
    chip: "bg-yellow-500/30 text-yellow-700",
    fill: "bg-yellow-500 text-white",
    icon: "text-yellow-500",
    pill: "bg-yellow-500 text-white",
    stripe: "bg-yellow-500",
    wash: "bg-yellow-500/30",
  },
};

function accent(name: AccentName): PreferenceAccent {
  return ACCENTS[name];
}

export const PREFERENCE_TAB_META: Record<PreferenceTabId, PreferenceTabMeta> = {
  about: {
    accent: accent("slate"),
    icon: "i-lucide:info",
  },
  data: {
    accent: accent("indigo"),
    icon: "i-lucide:database",
  },
  organize: {
    accent: accent("amber"),
    icon: "i-lucide:history",
  },
  record: {
    accent: accent("teal"),
    icon: "i-lucide:clipboard-plus",
  },
  reuse: {
    accent: accent("violet"),
    icon: "i-lucide:mouse-pointer-click",
  },
  shortcuts: {
    accent: accent("rose"),
    icon: "i-lucide:keyboard",
  },
  workflow: {
    accent: accent("sky"),
    icon: "i-lucide:panel-top",
  },
};

const SECTION_ACCENTS: Record<string, PreferenceAccent> = {
  appearance: accent("pink"),
  backup: accent("indigo"),
  capture: accent("teal"),
  control: accent("cyan"),
  copy: accent("fuchsia"),
  diagnostics: accent("red"),
  globalShortcuts: accent("rose"),
  groups: accent("lime"),
  history: accent("amber"),
  localData: accent("emerald"),
  organizing: accent("yellow"),
  paste: accent("violet"),
  permissions: accent("orange"),
  preview: accent("blue"),
  search: accent("cyan"),
  sensitive: accent("red"),
  source: accent("orange"),
  webdav: accent("sky"),
  window: accent("sky"),
};

export function resolveSectionAccent(sectionId: string): PreferenceAccent {
  return SECTION_ACCENTS[sectionId] ?? accent("teal");
}
