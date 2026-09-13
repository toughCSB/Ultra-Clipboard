import type { CSSProperties } from "react";
import type {
  ClipboardPreviewPayload,
  ClipboardPreviewRect,
  ClipboardPreviewState,
} from "@/commands";
import {
  PREVIEW_EMPTY_CONTENT_HEIGHT,
  PREVIEW_FILE_MORE_FOOTER_HEIGHT,
  PREVIEW_FILE_ROW_HEIGHT,
  PREVIEW_FILE_VERTICAL_PADDING,
  PREVIEW_PANEL_GAP,
  PREVIEW_PANEL_HEADER_HEIGHT,
  PREVIEW_PANEL_IMAGE_PADDING_X,
  PREVIEW_PANEL_IMAGE_PADDING_Y,
  PREVIEW_PANEL_MARGIN,
  PREVIEW_PANEL_MAX_HEIGHT,
  PREVIEW_PANEL_MAX_WIDTH,
  PREVIEW_PANEL_MIN_HEIGHT,
  PREVIEW_PANEL_MIN_WIDTH,
  PREVIEW_TEXT_ROW_HEIGHT,
  PREVIEW_TEXT_SOFT_WRAP_CHARS,
  PREVIEW_TEXT_VERTICAL_PADDING,
} from "./constants";
import type { PreviewMeasuredSize } from "./measurement";

/** Estimate image panel size from stored dimensions before image loading completes. */
export function resolveEffectivePanelSize(
  layout: ClipboardPreviewState["layout"],
  measuredSize: PreviewMeasuredSize,
  payload: ClipboardPreviewPayload | null,
): PreviewMeasuredSize {
  if (!payload) return measuredSize;

  if (payload.kind === "text") {
    return resolveTextPanelSize(layout, payload.text ?? "");
  }

  if (payload.kind === "files") {
    return resolveFilesPanelSize(
      layout,
      payload.files.length,
      payload.totalFiles,
    );
  }

  if (payload.kind !== "image") return measuredSize;

  const imageSize = resolveImagePanelSize(
    layout,
    payload.imageWidth,
    payload.imageHeight,
  );
  if (!imageSize) return measuredSize;

  return imageSize;
}

/** Build the panel rect from natural content size while preserving placement bounds. */
export function resolveDynamicPanelRect(
  layout: ClipboardPreviewState["layout"],
  measuredSize: PreviewMeasuredSize,
) {
  const width = clamp(
    measuredSize.width,
    PREVIEW_PANEL_MIN_WIDTH,
    Math.min(PREVIEW_PANEL_MAX_WIDTH, layout.panelRect.width),
  );
  const height = clamp(
    measuredSize.height,
    PREVIEW_PANEL_MIN_HEIGHT,
    Math.min(PREVIEW_PANEL_MAX_HEIGHT, layout.panelRect.height),
  );
  const raw = rawDynamicPanelRect(layout, width, height);

  return clampRect(raw, insetRect(layout.overlayRect, PREVIEW_PANEL_MARGIN));
}

/** Keep the hidden measurement layer's width cap aligned with the real panel. */
export function resolveMeasurePanelStyle(
  layout: ClipboardPreviewState["layout"],
): CSSProperties {
  const maxWidth = Math.min(PREVIEW_PANEL_MAX_WIDTH, layout.panelRect.width);
  const minWidth = Math.min(PREVIEW_PANEL_MIN_WIDTH, maxWidth);

  return {
    maxWidth,
    minWidth,
  };
}

/** Convert a cross-platform rect into React absolute-positioning styles. */
export function rectStyle(rect: ClipboardPreviewRect) {
  return {
    height: rect.height,
    left: rect.left,
    top: rect.top,
    width: rect.width,
  };
}

/** Place the dynamic panel on the source item's side and align its center. */
function rawDynamicPanelRect(
  layout: ClipboardPreviewState["layout"],
  width: number,
  height: number,
) {
  const sourceRect = layout.sourceRect;
  const centeredTop = sourceRect.top + sourceRect.height / 2 - height / 2;
  const centeredLeft = sourceRect.left + sourceRect.width / 2 - width / 2;

  switch (layout.placement) {
    case "right":
      return {
        height,
        left: sourceRect.left + sourceRect.width + PREVIEW_PANEL_GAP,
        top: centeredTop,
        width,
      };
    case "left":
      return {
        height,
        left: sourceRect.left - PREVIEW_PANEL_GAP - width,
        top: centeredTop,
        width,
      };
    case "bottom":
      return {
        height,
        left: centeredLeft,
        top: sourceRect.top + sourceRect.height + PREVIEW_PANEL_GAP,
        width,
      };
    case "top":
      return {
        height,
        left: centeredLeft,
        top: sourceRect.top - PREVIEW_PANEL_GAP - height,
        width,
      };
  }
}

/** Calculate proportional image display size from source dimensions and panel limits. */
function resolveImagePanelSize(
  layout: ClipboardPreviewState["layout"],
  imageWidth: number | null,
  imageHeight: number | null,
): PreviewMeasuredSize | null {
  if (!imageWidth || !imageHeight || imageWidth <= 0 || imageHeight <= 0) {
    return null;
  }

  const maxPanelWidth = Math.min(
    PREVIEW_PANEL_MAX_WIDTH,
    layout.panelRect.width,
  );
  const maxPanelHeight = Math.min(
    PREVIEW_PANEL_MAX_HEIGHT,
    layout.panelRect.height,
  );
  const maxImageWidth = Math.max(
    1,
    maxPanelWidth - PREVIEW_PANEL_IMAGE_PADDING_X,
  );
  const maxImageHeight = Math.max(
    1,
    maxPanelHeight -
      PREVIEW_PANEL_HEADER_HEIGHT -
      PREVIEW_PANEL_IMAGE_PADDING_Y,
  );
  const scale = Math.min(
    1,
    maxImageWidth / imageWidth,
    maxImageHeight / imageHeight,
  );

  return {
    height: clamp(
      Math.ceil(imageHeight * scale) +
        PREVIEW_PANEL_HEADER_HEIGHT +
        PREVIEW_PANEL_IMAGE_PADDING_Y,
      PREVIEW_PANEL_MIN_HEIGHT,
      maxPanelHeight,
    ),
    width: clamp(
      Math.ceil(imageWidth * scale) + PREVIEW_PANEL_IMAGE_PADDING_X,
      PREVIEW_PANEL_MIN_WIDTH,
      maxPanelWidth,
    ),
  };
}

/** Estimate text panel size using the same soft-wrap rules as the text viewer. */
function resolveTextPanelSize(
  layout: ClipboardPreviewState["layout"],
  text: string,
): PreviewMeasuredSize {
  const maxPanelWidth = Math.min(
    PREVIEW_PANEL_MAX_WIDTH,
    layout.panelRect.width,
  );
  const maxPanelHeight = Math.min(
    PREVIEW_PANEL_MAX_HEIGHT,
    layout.panelRect.height,
  );
  const rowCount = countTextPreviewRows(text);
  const contentHeight =
    rowCount === 0
      ? PREVIEW_EMPTY_CONTENT_HEIGHT
      : rowCount * PREVIEW_TEXT_ROW_HEIGHT + PREVIEW_TEXT_VERTICAL_PADDING;

  return {
    height: clamp(
      PREVIEW_PANEL_HEADER_HEIGHT + contentHeight,
      PREVIEW_PANEL_MIN_HEIGHT,
      maxPanelHeight,
    ),
    width: clamp(maxPanelWidth, PREVIEW_PANEL_MIN_WIDTH, maxPanelWidth),
  };
}

/** Estimate file panel size from returned rows and truncation state. */
function resolveFilesPanelSize(
  layout: ClipboardPreviewState["layout"],
  shownCount: number,
  totalCount: number,
): PreviewMeasuredSize {
  const maxPanelWidth = Math.min(
    PREVIEW_PANEL_MAX_WIDTH,
    layout.panelRect.width,
  );
  const maxPanelHeight = Math.min(
    PREVIEW_PANEL_MAX_HEIGHT,
    layout.panelRect.height,
  );
  const contentHeight =
    shownCount === 0
      ? PREVIEW_EMPTY_CONTENT_HEIGHT
      : shownCount * PREVIEW_FILE_ROW_HEIGHT +
        PREVIEW_FILE_VERTICAL_PADDING +
        (totalCount > shownCount ? PREVIEW_FILE_MORE_FOOTER_HEIGHT : 0);

  return {
    height: clamp(
      PREVIEW_PANEL_HEADER_HEIGHT + contentHeight,
      PREVIEW_PANEL_MIN_HEIGHT,
      maxPanelHeight,
    ),
    width: clamp(maxPanelWidth, PREVIEW_PANEL_MIN_WIDTH, maxPanelWidth),
  };
}

function countTextPreviewRows(text: string) {
  if (text.length === 0) return 0;

  let rowCount = 0;
  for (const line of text.split("\n")) {
    rowCount += Math.max(
      1,
      Math.ceil(line.length / PREVIEW_TEXT_SOFT_WRAP_CHARS),
    );
  }

  return rowCount;
}

function clampRect(rect: ClipboardPreviewRect, bounds: ClipboardPreviewRect) {
  const maxLeft = Math.max(bounds.left, rectRight(bounds) - rect.width);
  const maxTop = Math.max(bounds.top, rectBottom(bounds) - rect.height);

  return {
    height: rect.height,
    left: clamp(rect.left, bounds.left, maxLeft),
    top: clamp(rect.top, bounds.top, maxTop),
    width: rect.width,
  };
}

function insetRect(rect: ClipboardPreviewRect, amount: number) {
  return {
    height: Math.max(1, rect.height - amount * 2),
    left: rect.left + amount,
    top: rect.top + amount,
    width: Math.max(1, rect.width - amount * 2),
  };
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function rectRight(rect: { left: number; width: number }) {
  return rect.left + rect.width;
}

function rectBottom(rect: { top: number; height: number }) {
  return rect.top + rect.height;
}
