import { Button, ColorPicker, Segmented, Slider, Switch } from "antd";
import type { CSSProperties, FC, ReactNode } from "react";
import { useTranslation } from "react-i18next";
import Tooltip from "@/components/Tooltip";
import { cn } from "@/utils/cn";
import {
  BACKDROP_BACKGROUNDS,
  type Backdrop,
  type BackdropBackground,
} from "../model/document";
import type { BlurMode, ShapeKind, TextStyle } from "../model/shapes";
import {
  COLOR_PRESETS,
  COUNTER_SIZE_RANGE,
  type EditorTool,
  HIGHLIGHT_PRESETS,
  MAGNIFIER_ZOOMS,
  STROKE_RANGE,
  TEXT_SIZE_RANGE,
  type ToolStyle,
} from "../model/tools";
import { BACKDROP_GRADIENTS } from "../render/drawDocument";
import type { ToolAnchorRect } from "./EditorToolbar";

export type OptionsSubject = EditorTool | ShapeKind;

interface ToolOptionsProps {
  /** The active tool's own toolbar button, so the panel opens right under it. */
  anchor: ToolAnchorRect | null;
  backdrop: Backdrop | null;
  /** Largest padding and radius the backdrop sliders offer, in image pixels. */
  backdropLimit: number;
  hasSelection: boolean;
  style: ToolStyle;
  subject: OptionsSubject;
  onBackdropChange: (backdrop: Backdrop, final: boolean) => void;
  onBackdropToggle: (enabled: boolean) => void;
  onDelete: () => void;
  onStyleChange: (patch: Partial<ToolStyle>, final: boolean) => void;
}

const PANEL_WIDTH = 264;
const VIEWPORT_MARGIN = 8;
const ANCHOR_GAP = 6;

const COLOR_SUBJECTS = new Set<OptionsSubject>([
  "arrow",
  "counter",
  "line",
  "oval",
  "pen",
  "rect",
  "ruler",
  "text",
]);

const WIDTH_SUBJECTS = new Set<OptionsSubject>([
  "arrow",
  "line",
  "oval",
  "pen",
  "rect",
  "ruler",
]);

/** Label to the left, control to the right — one line instead of two. */
const Row: FC<{ children: ReactNode; label: string }> = ({
  children,
  label,
}) => {
  return (
    <div className="flex items-center gap-2">
      <span className="w-11 shrink-0 text-ant-secondary text-xs">{label}</span>
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  );
};

const Swatches: FC<{
  colors: readonly string[];
  customLabel: string;
  value: string;
  onChange: (color: string) => void;
}> = ({ colors, customLabel, value, onChange }) => {
  return (
    <div className="flex flex-wrap items-center gap-1">
      {colors.map((color) => {
        const active = color.toUpperCase() === value.toUpperCase();

        return (
          <button
            aria-label={color}
            aria-pressed={active}
            className={cn(
              "h-5 w-5 cursor-pointer rounded-full border border-ant-border p-0 transition-transform hover:scale-110",
              { "ring-2 ring-ant-primary ring-offset-1": active },
            )}
            key={color}
            onClick={() => onChange(color)}
            style={{ backgroundColor: color }}
            type="button"
          />
        );
      })}
      <Tooltip title={customLabel}>
        <ColorPicker
          disabledAlpha
          onChangeComplete={(color) => {
            onChange(color.toHexString().toUpperCase());
          }}
          size="small"
          value={value}
        />
      </Tooltip>
    </div>
  );
};

/** Small floating panel with the options of the active tool or selected annotation. */
const ToolOptions: FC<ToolOptionsProps> = (props) => {
  const { t } = useTranslation("screenshot");
  const {
    anchor,
    backdrop,
    backdropLimit,
    hasSelection,
    style,
    subject,
    onBackdropChange,
    onBackdropToggle,
    onDelete,
    onStyleChange,
  } = props;

  const change = (patch: Partial<ToolStyle>) => {
    onStyleChange(patch, true);
  };

  const sections: ReactNode[] = [];

  if (subject === "backdrop") {
    const current = backdrop;

    sections.push(
      <div className="flex items-center justify-between" key="enabled">
        <span className="text-xs">{t("editor.options.backdropEnabled")}</span>
        <Switch
          checked={current !== null}
          onChange={onBackdropToggle}
          size="small"
        />
      </div>,
    );

    if (current) {
      sections.push(
        <Row key="background" label={t("editor.options.background")}>
          <div className="flex flex-wrap gap-1.5">
            {BACKDROP_BACKGROUNDS.map((background: BackdropBackground) => {
              const stops = BACKDROP_GRADIENTS[background];
              const active = current.background === background;

              return (
                <Tooltip
                  key={background}
                  title={t(`editor.options.backgrounds.${background}`)}
                >
                  <button
                    aria-label={t(`editor.options.backgrounds.${background}`)}
                    aria-pressed={active}
                    className={cn(
                      "h-6 w-6 cursor-pointer rounded-2 border border-ant-border p-0",
                      { "ring-2 ring-ant-primary ring-offset-1": active },
                    )}
                    onClick={() => {
                      onBackdropChange({ ...current, background }, true);
                    }}
                    style={{
                      background:
                        stops.length > 0
                          ? `linear-gradient(135deg, ${stops.join(", ")})`
                          : "repeating-conic-gradient(#d4d4d8 0% 25%, #ffffff 0% 50%) 50% / 10px 10px",
                    }}
                    type="button"
                  />
                </Tooltip>
              );
            })}
          </div>
        </Row>,
        <Row key="padding" label={t("editor.options.padding")}>
          <Slider
            max={backdropLimit}
            min={0}
            onChange={(padding) => {
              onBackdropChange({ ...current, padding }, false);
            }}
            onChangeComplete={(padding) => {
              onBackdropChange({ ...current, padding }, true);
            }}
            value={current.padding}
          />
        </Row>,
        <Row key="radius" label={t("editor.options.radius")}>
          <Slider
            max={Math.max(8, Math.round(backdropLimit / 3))}
            min={0}
            onChange={(radius) => {
              onBackdropChange({ ...current, radius }, false);
            }}
            onChangeComplete={(radius) => {
              onBackdropChange({ ...current, radius }, true);
            }}
            value={current.radius}
          />
        </Row>,
        <div className="flex items-center justify-between" key="shadow">
          <span className="text-xs">{t("editor.options.shadow")}</span>
          <Switch
            checked={current.shadow}
            onChange={(shadow) => {
              onBackdropChange({ ...current, shadow }, true);
            }}
            size="small"
          />
        </div>,
      );
    }
  }

  if (COLOR_SUBJECTS.has(subject)) {
    sections.push(
      <Row key="color" label={t("editor.options.color")}>
        <Swatches
          colors={COLOR_PRESETS}
          customLabel={t("editor.options.customColor")}
          onChange={(color) => change({ color })}
          value={style.color}
        />
      </Row>,
    );
  }

  if (subject === "highlighter") {
    sections.push(
      <Row key="highlight" label={t("editor.options.color")}>
        <Swatches
          colors={HIGHLIGHT_PRESETS}
          customLabel={t("editor.options.customColor")}
          onChange={(highlightColor) => change({ highlightColor })}
          value={style.highlightColor}
        />
      </Row>,
    );
  }

  if (subject === "rect" || subject === "oval") {
    sections.push(
      <Row key="fill" label={t("editor.options.fill")}>
        <Segmented<"stroke" | "fill">
          block
          onChange={(value) => change({ rectFill: value === "fill" })}
          options={[
            { label: t("editor.options.stroke"), value: "stroke" },
            { label: t("editor.options.filled"), value: "fill" },
          ]}
          size="small"
          value={style.rectFill ? "fill" : "stroke"}
        />
      </Row>,
    );
  }

  const filledBox =
    (subject === "rect" || subject === "oval") && style.rectFill;

  if (WIDTH_SUBJECTS.has(subject) && !filledBox) {
    sections.push(
      <Row key="width" label={t("editor.options.width")}>
        <Slider
          max={STROKE_RANGE.max}
          min={STROKE_RANGE.min}
          onChange={(strokeWidth) => onStyleChange({ strokeWidth }, false)}
          onChangeComplete={(strokeWidth) => change({ strokeWidth })}
          step={0.5}
          value={style.strokeWidth}
        />
      </Row>,
    );
  }

  if (subject === "text") {
    sections.push(
      <Row key="textSize" label={t("editor.options.textSize")}>
        <Slider
          max={TEXT_SIZE_RANGE.max}
          min={TEXT_SIZE_RANGE.min}
          onChange={(textSize) => onStyleChange({ textSize }, false)}
          onChangeComplete={(textSize) => change({ textSize })}
          value={style.textSize}
        />
      </Row>,
      <Row key="textStyle" label={t("editor.options.textStyle")}>
        <Segmented<TextStyle>
          block
          onChange={(textStyle) => change({ textStyle })}
          options={[
            { label: t("editor.options.stylePlain"), value: "plain" },
            { label: t("editor.options.styleOutline"), value: "outline" },
            { label: t("editor.options.styleBackground"), value: "background" },
          ]}
          size="small"
          value={style.textStyle}
        />
      </Row>,
    );
  }

  if (subject === "blur") {
    sections.push(
      <Row key="blurMode" label={t("editor.options.blurMode")}>
        <Segmented<BlurMode>
          block
          onChange={(blurMode) => change({ blurMode })}
          options={[
            { label: t("editor.options.blur"), value: "blur" },
            { label: t("editor.options.pixelate"), value: "pixelate" },
            { label: t("editor.options.erase"), value: "erase" },
          ]}
          size="small"
          value={style.blurMode}
        />
      </Row>,
    );

    if (style.blurMode !== "erase") {
      sections.push(
        <Row key="strength" label={t("editor.options.strength")}>
          <Slider
            max={STROKE_RANGE.max}
            min={STROKE_RANGE.min}
            onChange={(strokeWidth) => onStyleChange({ strokeWidth }, false)}
            onChangeComplete={(strokeWidth) => change({ strokeWidth })}
            value={style.strokeWidth}
          />
        </Row>,
      );
    }
  }

  if (subject === "spotlight") {
    sections.push(
      <Row key="spotlight" label={t("editor.options.shape")}>
        <Segmented<"rect" | "oval">
          block
          onChange={(value) => change({ spotlightOval: value === "oval" })}
          options={[
            { label: t("editor.tools.rect"), value: "rect" },
            { label: t("editor.tools.oval"), value: "oval" },
          ]}
          size="small"
          value={style.spotlightOval ? "oval" : "rect"}
        />
      </Row>,
    );
  }

  if (subject === "counter") {
    sections.push(
      <Row key="counterSize" label={t("editor.options.size")}>
        <Slider
          max={COUNTER_SIZE_RANGE.max}
          min={COUNTER_SIZE_RANGE.min}
          onChange={(counterSize) => onStyleChange({ counterSize }, false)}
          onChangeComplete={(counterSize) => change({ counterSize })}
          value={style.counterSize}
        />
      </Row>,
    );
  }

  if (subject === "magnifier") {
    sections.push(
      <Row key="zoom" label={t("editor.options.zoom")}>
        <Segmented<number>
          block
          onChange={(magnifierZoom) => change({ magnifierZoom })}
          options={MAGNIFIER_ZOOMS.map((zoom) => {
            return { label: `${zoom}×`, value: zoom };
          })}
          size="small"
          value={style.magnifierZoom}
        />
      </Row>,
    );
  }

  if (sections.length === 0 && !hasSelection) return null;

  // Anchored under the active tool's own button when known; a fixed corner
  // otherwise (e.g. options for a selected shape while "select" is active).
  const panelStyle: CSSProperties = anchor
    ? {
        left: Math.min(
          Math.max(anchor.left, VIEWPORT_MARGIN),
          window.innerWidth - PANEL_WIDTH - VIEWPORT_MARGIN,
        ),
        position: "fixed",
        top: anchor.bottom + ANCHOR_GAP,
        width: PANEL_WIDTH,
      }
    : { left: 12, position: "absolute", top: 12, width: PANEL_WIDTH };

  return (
    <section
      aria-label={t(`editor.tools.${subject}`)}
      // Translucent + blurred so the panel reads as an overlay, not a card
      // that hides whatever is right under the tool icon.
      className="z-10 flex flex-col gap-1.5 rounded-3 border border-white/10 bg-[#161618]/40 p-2.5 shadow-lg backdrop-blur-md"
      data-editor-options="true"
      style={panelStyle}
    >
      <header className="flex items-center justify-between gap-2">
        <span className="font-semibold text-xs">
          {t(`editor.tools.${subject}`)}
        </span>
        {hasSelection && (
          <Tooltip title={`${t("editor.delete")} (Delete)`}>
            <Button
              aria-label={t("editor.delete")}
              danger
              icon={<span className="i-lucide:trash-2" />}
              onClick={onDelete}
              size="small"
              type="text"
            />
          </Tooltip>
        )}
      </header>
      {sections}
    </section>
  );
};

export default ToolOptions;
