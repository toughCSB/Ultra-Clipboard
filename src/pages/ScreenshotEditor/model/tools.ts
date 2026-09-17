import type { BlurMode, TextStyle } from "./shapes";

export type EditorTool =
  | "select"
  | "arrow"
  | "text"
  | "ruler"
  | "rect"
  | "backdrop"
  | "pen"
  | "magnifier"
  | "blur"
  | "highlighter"
  | "spotlight"
  | "counter"
  | "oval"
  | "line";

export interface ToolDefinition {
  icon: string;
  id: EditorTool;
  /** Single-key shortcut, matched against `KeyboardEvent.code`. */
  code: string;
  key: string;
  /** Per-tool accent so the toolbar reads at a glance, not just on hover. */
  color: string;
}

/**
 * All drawing tools, shown directly in the toolbar. This is only the
 * fallback order — the toolbar lets a user drag icons to their own order,
 * stored separately by `loadToolOrder`/`saveToolOrder`.
 */
export const ALL_TOOLS: readonly ToolDefinition[] = [
  {
    code: "KeyV",
    color: "#9CA3AF",
    icon: "i-lucide:square-dashed-mouse-pointer",
    id: "select",
    key: "V",
  },
  {
    code: "KeyA",
    color: "#F87171",
    icon: "i-lucide:move-up-right",
    id: "arrow",
    key: "A",
  },
  {
    code: "KeyT",
    color: "#60A5FA",
    icon: "i-lucide:type",
    id: "text",
    key: "T",
  },
  {
    code: "KeyR",
    color: "#4ADE80",
    icon: "i-lucide:square",
    id: "rect",
    key: "R",
  },
  {
    code: "KeyH",
    color: "#FACC15",
    icon: "i-lucide:highlighter",
    id: "highlighter",
    key: "H",
  },
  {
    code: "KeyS",
    color: "#C084FC",
    icon: "i-lucide:flashlight",
    id: "spotlight",
    key: "S",
  },
  {
    code: "KeyB",
    color: "#67E8F9",
    icon: "i-lucide:droplets",
    id: "blur",
    key: "B",
  },
  {
    code: "KeyP",
    color: "#FB923C",
    icon: "i-lucide:pen-line",
    id: "pen",
    key: "P",
  },
  {
    code: "KeyO",
    color: "#2DD4BF",
    icon: "i-lucide:circle",
    id: "oval",
    key: "O",
  },
  {
    code: "KeyL",
    color: "#818CF8",
    icon: "i-lucide:slash",
    id: "line",
    key: "L",
  },
  {
    code: "KeyC",
    color: "#FB7185",
    icon: "i-lucide:circle-dot",
    id: "counter",
    key: "C",
  },
  {
    code: "KeyU",
    color: "#A78BFA",
    icon: "i-lucide:ruler",
    id: "ruler",
    key: "U",
  },
  {
    code: "KeyK",
    color: "#F472B6",
    icon: "i-lucide:wallpaper",
    id: "backdrop",
    key: "K",
  },
  {
    code: "KeyM",
    color: "#38BDF8",
    icon: "i-lucide:zoom-in",
    id: "magnifier",
    key: "M",
  },
];

export const DEFAULT_TOOL_ORDER: readonly EditorTool[] = ALL_TOOLS.map(
  (tool) => tool.id,
);

export const toolByCode = (code: string) => {
  return ALL_TOOLS.find((tool) => tool.code === code) ?? null;
};

export const toolDefinition = (id: EditorTool) => {
  return ALL_TOOLS.find((tool) => tool.id === id) ?? ALL_TOOLS[0];
};

const ORDER_STORAGE_KEY = "screenshot-editor:tool-order";

/**
 * Validates a stored tool order: drops ids that no longer exist, and appends
 * any tool the stored order is missing (e.g. one added after the order was
 * saved) at the end, so every known tool always renders exactly once.
 */
export const parseToolOrder = (value: unknown): EditorTool[] => {
  const known = new Set(DEFAULT_TOOL_ORDER);
  const fromStorage = Array.isArray(value)
    ? value.filter((id): id is EditorTool => known.has(id as EditorTool))
    : [];
  const seen = new Set(fromStorage);
  const missing = DEFAULT_TOOL_ORDER.filter((id) => !seen.has(id));

  return [...fromStorage, ...missing];
};

export const loadToolOrder = (): EditorTool[] => {
  try {
    const raw = localStorage.getItem(ORDER_STORAGE_KEY);

    return raw ? parseToolOrder(JSON.parse(raw)) : [...DEFAULT_TOOL_ORDER];
  } catch {
    return [...DEFAULT_TOOL_ORDER];
  }
};

export const saveToolOrder = (order: readonly EditorTool[]) => {
  try {
    localStorage.setItem(ORDER_STORAGE_KEY, JSON.stringify(order));
  } catch {
    // Storage can be unavailable; the order still applies to this editor.
  }
};

/** Last used drawing options. Sizes are logical pixels, scaled by the capture's DPI. */
export interface ToolStyle {
  blurMode: BlurMode;
  color: string;
  counterSize: number;
  highlightColor: string;
  magnifierZoom: number;
  rectFill: boolean;
  spotlightOval: boolean;
  strokeWidth: number;
  textSize: number;
  textStyle: TextStyle;
}

export const COLOR_PRESETS = [
  "#FF3B30",
  "#FF9500",
  "#FFCC00",
  "#34C759",
  "#007AFF",
  "#AF52DE",
  "#000000",
  "#FFFFFF",
] as const;

export const HIGHLIGHT_PRESETS = [
  "#FFE600",
  "#7CFC8A",
  "#6EE7FF",
  "#FF9BD2",
  "#FFB74D",
] as const;

export const STROKE_RANGE = { max: 24, min: 1 } as const;
export const TEXT_SIZE_RANGE = { max: 96, min: 10 } as const;
export const COUNTER_SIZE_RANGE = { max: 48, min: 8 } as const;
export const MAGNIFIER_ZOOMS = [2, 3, 4] as const;

export const DEFAULT_TOOL_STYLE: ToolStyle = {
  blurMode: "blur",
  color: COLOR_PRESETS[0],
  counterSize: 14,
  highlightColor: HIGHLIGHT_PRESETS[0],
  magnifierZoom: 2,
  rectFill: false,
  spotlightOval: false,
  strokeWidth: 4,
  textSize: 24,
  textStyle: "plain",
};

const STORAGE_KEY = "screenshot-editor:tool-style";

const isHexColor = (value: unknown): value is string => {
  return typeof value === "string" && /^#[0-9a-f]{6}$/i.test(value);
};

const clampNumber = (
  value: unknown,
  range: { min: number; max: number },
  fallback: number,
) => {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;

  return Math.min(range.max, Math.max(range.min, value));
};

/** Validates stored options field by field so a bad value never breaks the editor. */
export const parseToolStyle = (value: unknown): ToolStyle => {
  if (typeof value !== "object" || value === null) return DEFAULT_TOOL_STYLE;

  const stored = value as Partial<Record<keyof ToolStyle, unknown>>;
  const fallback = DEFAULT_TOOL_STYLE;

  return {
    blurMode:
      stored.blurMode === "blur" ||
      stored.blurMode === "pixelate" ||
      stored.blurMode === "erase"
        ? stored.blurMode
        : fallback.blurMode,
    color: isHexColor(stored.color) ? stored.color : fallback.color,
    counterSize: clampNumber(
      stored.counterSize,
      COUNTER_SIZE_RANGE,
      fallback.counterSize,
    ),
    highlightColor: isHexColor(stored.highlightColor)
      ? stored.highlightColor
      : fallback.highlightColor,
    magnifierZoom: MAGNIFIER_ZOOMS.includes(
      stored.magnifierZoom as (typeof MAGNIFIER_ZOOMS)[number],
    )
      ? (stored.magnifierZoom as number)
      : fallback.magnifierZoom,
    rectFill:
      typeof stored.rectFill === "boolean"
        ? stored.rectFill
        : fallback.rectFill,
    spotlightOval:
      typeof stored.spotlightOval === "boolean"
        ? stored.spotlightOval
        : fallback.spotlightOval,
    strokeWidth: clampNumber(
      stored.strokeWidth,
      STROKE_RANGE,
      fallback.strokeWidth,
    ),
    textSize: clampNumber(stored.textSize, TEXT_SIZE_RANGE, fallback.textSize),
    textStyle:
      stored.textStyle === "plain" ||
      stored.textStyle === "outline" ||
      stored.textStyle === "background"
        ? stored.textStyle
        : fallback.textStyle,
  };
};

export const loadToolStyle = () => {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);

    return raw ? parseToolStyle(JSON.parse(raw)) : DEFAULT_TOOL_STYLE;
  } catch {
    return DEFAULT_TOOL_STYLE;
  }
};

export const saveToolStyle = (style: ToolStyle) => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(style));
  } catch {
    // Storage can be unavailable; the options still apply to this editor.
  }
};
