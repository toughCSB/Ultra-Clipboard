import { Button } from "antd";
import type { FC, PointerEvent as ReactPointerEvent, RefObject } from "react";
import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { ScreenshotTextRegion } from "@/commands";
import type { PixelRect } from "@/utils/pixelRect";
import {
  textFromRegionRange,
  textRegionScreenRect,
} from "../model/textSelection";
import type { Viewport } from "../model/viewport";

interface ImageTextSelectionProps {
  area: PixelRect;
  loading: boolean;
  ratio: number;
  regions: readonly ScreenshotTextRegion[];
  rootRef: RefObject<HTMLDivElement | null>;
  selectedText: string;
  viewport: Viewport;
  onCopy: () => void;
  onLeave: () => void;
  onSelectedText: (text: string) => void;
}

const endpointOf = (node: Node | null, offset: number, root: HTMLElement) => {
  const element = node instanceof Element ? node : node?.parentElement;
  const span = element?.closest<HTMLElement>("[data-text-region]");
  if (!span || !root.contains(span)) return null;

  const index = Number(span.dataset.textRegion);
  if (!Number.isInteger(index)) return null;

  return {
    index,
    offset:
      node?.nodeType === Node.TEXT_NODE
        ? offset
        : offset > 0
          ? (span.textContent?.length ?? 0)
          : 0,
  };
};

export const selectedImageText = (
  regions: readonly ScreenshotTextRegion[],
  root: HTMLElement | null,
) => {
  const selection = window.getSelection();
  if (!root || !selection || selection.isCollapsed) return "";

  const anchor = endpointOf(selection.anchorNode, selection.anchorOffset, root);
  const focus = endpointOf(selection.focusNode, selection.focusOffset, root);
  if (!anchor || !focus) return "";

  return textFromRegionRange(regions, anchor, focus);
};

/** Places selectable DOM words over the rendered editor view without changing its image. */
const ImageTextSelection: FC<ImageTextSelectionProps> = (props) => {
  const { t } = useTranslation("screenshot");
  const {
    area,
    loading,
    ratio,
    regions,
    rootRef,
    selectedText,
    viewport,
    onCopy,
    onLeave,
    onSelectedText,
  } = props;
  const measuredWidths = useMemo(() => {
    const context = document.createElement("canvas").getContext("2d");

    return regions.map((region) => {
      if (!context) return 0;
      context.font = `${Math.max(1, region.height * 0.85)}px Arial, sans-serif`;
      return context.measureText(region.text.trimEnd()).width;
    });
  }, [regions]);
  const keyedRegions = useMemo(() => {
    const counts = new Map<string, number>();

    return regions.map((region, index) => {
      const signature = JSON.stringify([
        region.lineIndex,
        region.x,
        region.y,
        region.width,
        region.height,
        region.text,
      ]);
      const occurrence = (counts.get(signature) ?? 0) + 1;
      counts.set(signature, occurrence);

      return { index, key: `${signature}:${occurrence}`, region };
    });
  }, [regions]);

  useEffect(() => {
    const handleSelectionChange = () => {
      const selected = selectedImageText(regions, rootRef.current);
      if (selected) onSelectedText(selected);
    };
    document.addEventListener("selectionchange", handleSelectionChange);

    return () => {
      document.removeEventListener("selectionchange", handleSelectionChange);
    };
  }, [onSelectedText, regions, rootRef]);

  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.target instanceof Element && event.target.closest("button"))
      return;
    onSelectedText("");
  };

  return (
    <div
      className="absolute inset-0 z-10 cursor-text select-text"
      onPointerDown={handlePointerDown}
      ref={rootRef}
    >
      {keyedRegions.map(({ index, key, region }) => {
        const rect = textRegionScreenRect(region, area, viewport, ratio);
        const measured = measuredWidths[index];
        const horizontalScale = measured > 0 ? region.width / measured : 1;

        return (
          <span
            className="absolute block select-text overflow-hidden whitespace-nowrap text-transparent hover:outline hover:outline-1 hover:outline-ant-primary"
            data-text-region={index}
            key={key}
            style={{
              height: rect.height,
              left: rect.left,
              top: rect.top,
              width: rect.width,
            }}
          >
            <span
              className="inline-block whitespace-pre"
              style={{
                fontFamily: "Arial, sans-serif",
                fontSize: Math.max(1, rect.height * 0.85),
                lineHeight: 1,
                transform: `scaleX(${horizontalScale})`,
                transformOrigin: "left top",
              }}
            >
              {region.text}
            </span>
          </span>
        );
      })}
      <div className="absolute top-3 right-3 z-20 flex items-center gap-2 rounded-2 border border-ant-border bg-ant-container p-2 shadow-md">
        <span className="max-w-52 text-ant-secondary text-xs">
          {loading
            ? t("editor.textSelect.loading")
            : regions.length === 0
              ? t("editor.textSelect.none")
              : selectedText.trim()
                ? t("editor.textSelect.ready")
                : t("editor.textSelect.hint")}
        </span>
        <Button
          disabled={loading || !selectedText.trim()}
          onClick={onCopy}
          size="small"
        >
          {t("editor.textSelect.copy")}
        </Button>
        <Button onClick={onLeave} size="small" type="primary">
          {t("editor.textSelect.back")}
        </Button>
      </div>
    </div>
  );
};

export default ImageTextSelection;
