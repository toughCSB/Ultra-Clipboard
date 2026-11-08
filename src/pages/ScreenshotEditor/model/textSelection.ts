import type { ScreenshotTextRegion } from "@/commands";
import type { PixelRect } from "@/utils/pixelRect";
import type { Viewport } from "./viewport";

export interface TextEndpoint {
  index: number;
  offset: number;
}

export const textFromRegionRange = (
  regions: readonly ScreenshotTextRegion[],
  anchor: TextEndpoint,
  focus: TextEndpoint,
) => {
  const [start, end] =
    anchor.index < focus.index ||
    (anchor.index === focus.index && anchor.offset <= focus.offset)
      ? [anchor, focus]
      : [focus, anchor];
  let selected = "";
  let previousLine: number | null = null;

  for (let index = start.index; index <= end.index; index += 1) {
    const region = regions[index];
    if (!region) continue;
    const from = index === start.index ? start.offset : 0;
    const to = index === end.index ? end.offset : region.text.length;
    const part = region.text.slice(from, to);
    if (!part) continue;

    if (previousLine !== null && previousLine !== region.lineIndex) {
      selected += "\n";
    }
    selected += part;
    previousLine = region.lineIndex;
  }

  return selected;
};

/** Converts exported-image pixels to the CSS viewport shared with EditorCanvas. */
export const textRegionScreenRect = (
  region: Pick<ScreenshotTextRegion, "height" | "width" | "x" | "y">,
  area: PixelRect,
  viewport: Viewport,
  ratio: number,
) => {
  const cssScale = viewport.zoom / ratio;

  return {
    height: region.height * cssScale,
    left: (area.x + region.x) * cssScale + viewport.offsetX / ratio,
    top: (area.y + region.y) * cssScale + viewport.offsetY / ratio,
    width: region.width * cssScale,
  };
};

export const validTextRegions = (
  regions: readonly ScreenshotTextRegion[] | null,
  area: PixelRect,
): ScreenshotTextRegion[] => {
  if (!regions) return [];

  return regions.flatMap((region) => {
    if (
      region.text.trim().length === 0 ||
      !Number.isInteger(region.lineIndex) ||
      ![region.x, region.y, region.width, region.height].every(Number.isFinite)
    ) {
      return [];
    }

    const x = Math.max(0, region.x);
    const y = Math.max(0, region.y);
    const right = Math.min(area.width, region.x + region.width);
    const bottom = Math.min(area.height, region.y + region.height);
    if (right <= x || bottom <= y) return [];

    return [{ ...region, height: bottom - y, width: right - x, x, y }];
  });
};
