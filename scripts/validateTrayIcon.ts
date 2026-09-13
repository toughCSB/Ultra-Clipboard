import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import {
  type AlphaRaster,
  parseIcoAlphaRasters,
  resizeAlpha,
} from "./iconAlpha.ts";

const ALPHA_THRESHOLD = 32;
const MAX_EDGE_MARGIN = 1;
const REQUIRED_EMBEDDED_SIZES = [16, 24, 32, 48] as const;
const VALIDATION_SIZES = [16, 20, 24, 32, 48] as const;

interface OccupancyResult {
  readonly bottom: number;
  readonly heightPercent: number;
  readonly left: number;
  readonly right: number;
  readonly sourceSize: number;
  readonly targetSize: number;
  readonly top: number;
  readonly widthPercent: number;
}

export interface TrayIconValidationReport {
  readonly embeddedSizes: readonly number[];
  readonly occupancy: readonly OccupancyResult[];
}

const measureOccupancy = (
  raster: AlphaRaster,
  sourceSize: number,
): OccupancyResult => {
  let minimumX = raster.width;
  let minimumY = raster.height;
  let maximumX = -1;
  let maximumY = -1;

  for (let y = 0; y < raster.height; y += 1) {
    for (let x = 0; x < raster.width; x += 1) {
      if ((raster.alpha[y * raster.width + x] ?? 0) < ALPHA_THRESHOLD) continue;

      minimumX = Math.min(minimumX, x);
      minimumY = Math.min(minimumY, y);
      maximumX = Math.max(maximumX, x);
      maximumY = Math.max(maximumY, y);
    }
  }

  if (maximumX < 0 || maximumY < 0) {
    throw new Error(
      `${raster.width}px raster has no alpha >= ${ALPHA_THRESHOLD}`,
    );
  }

  return {
    bottom: raster.height - maximumY - 1,
    heightPercent: ((maximumY - minimumY + 1) / raster.height) * 100,
    left: minimumX,
    right: raster.width - maximumX - 1,
    sourceSize,
    targetSize: raster.width,
    top: minimumY,
    widthPercent: ((maximumX - minimumX + 1) / raster.width) * 100,
  };
};

export const inspectTrayIcon = (iconPath: string): TrayIconValidationReport => {
  const rasters = parseIcoAlphaRasters(readFileSync(iconPath));
  const embeddedSizes = [...rasters.keys()].sort((left, right) => left - right);
  const occupancy = VALIDATION_SIZES.map((targetSize) => {
    const sourceSize =
      embeddedSizes.find((embeddedSize) => embeddedSize >= targetSize) ??
      embeddedSizes.at(-1);
    if (sourceSize === undefined) {
      throw new Error("ICO contains no image frames");
    }

    const source = rasters.get(sourceSize);
    if (source === undefined) {
      throw new Error(`ICO frame ${sourceSize}px is missing`);
    }

    return measureOccupancy(resizeAlpha(source, targetSize), sourceSize);
  });

  return { embeddedSizes, occupancy };
};

export const formatTrayIconReport = (report: TrayIconValidationReport) => {
  const lines = [
    `Embedded ICO sizes: ${report.embeddedSizes.join(", ")}px`,
    `Occupancy threshold: alpha >= ${ALPHA_THRESHOLD}`,
  ];
  for (const result of report.occupancy) {
    lines.push(
      `${result.targetSize}px (from ${result.sourceSize}px): ${result.widthPercent.toFixed(1)}% x ${result.heightPercent.toFixed(1)}%; margins L${result.left}/T${result.top}/R${result.right}/B${result.bottom}`,
    );
  }

  return lines.join("\n");
};

export const validateTrayIcon = (iconPath: string) => {
  const report = inspectTrayIcon(iconPath);
  const missingSizes = REQUIRED_EMBEDDED_SIZES.filter((size) => {
    return !report.embeddedSizes.includes(size);
  });
  const edgeFailures = report.occupancy.filter((result) => {
    return (
      result.left > MAX_EDGE_MARGIN ||
      result.top > MAX_EDGE_MARGIN ||
      result.right > MAX_EDGE_MARGIN ||
      result.bottom > MAX_EDGE_MARGIN
    );
  });

  if (missingSizes.length > 0 || edgeFailures.length > 0) {
    const failures: string[] = [];
    if (missingSizes.length > 0) {
      failures.push(`missing embedded sizes: ${missingSizes.join(", ")}px`);
    }
    for (const result of edgeFailures) {
      failures.push(
        `${result.targetSize}px exceeds ${MAX_EDGE_MARGIN}px edge margin (L${result.left}/T${result.top}/R${result.right}/B${result.bottom})`,
      );
    }

    throw new Error(`${formatTrayIconReport(report)}\n${failures.join("\n")}`);
  }

  return report;
};

const run = () => {
  const iconPath = resolve(process.argv[2] ?? "src-tauri/assets/tray.ico");
  const report = validateTrayIcon(iconPath);
  process.stdout.write(`${formatTrayIconReport(report)}\n`);
};

const entryPath = process.argv[1];
if (
  entryPath !== undefined &&
  import.meta.url === pathToFileURL(resolve(entryPath)).href
) {
  try {
    run();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`${message}\n`);
    process.exitCode = 1;
  }
}
