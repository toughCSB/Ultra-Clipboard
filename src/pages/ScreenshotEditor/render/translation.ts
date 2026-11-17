import type { ScreenshotTranslationBlock } from "@/commands";
import type { PixelRect } from "@/utils/pixelRect";
import type { Canvas2D } from "./drawDocument";

export interface TranslationPaintBlock {
  id: string;
  rect: PixelRect;
  background: string;
  foreground: string;
  fontSize: number;
  lineHeight: number;
  padding: number;
  lines: readonly string[];
  overflow: boolean;
}

export interface TranslationLayer {
  area: PixelRect;
  blocks: readonly TranslationPaintBlock[];
  overflowCount: number;
}

type Measure = (text: string, fontSize: number) => number;
const FONT = "sans-serif";

export const wrapTranslationText = (
  text: string,
  width: number,
  fontSize: number,
  measure: Measure,
): string[] => {
  const lines: string[] = [];
  for (const paragraph of text.replace(/\r\n?/g, "\n").split("\n")) {
    let line = "";
    for (const token of paragraph.match(/\s+|\S+/gu) ?? []) {
      if (measure(line + token, fontSize) <= width) {
        line += token;
        continue;
      }
      if (line.trim()) lines.push(line.trimEnd());
      line = "";
      const word = token.trimStart();
      for (const character of Array.from(word)) {
        if (line && measure(line + character, fontSize) > width) {
          lines.push(line);
          line = "";
        }
        line += character;
      }
    }
    lines.push(line.trimEnd());
  }
  return lines;
};

export const fitTranslationText = (
  block: ScreenshotTranslationBlock,
  maxHeight: number,
  measure: Measure,
) => {
  const padding = Math.min(3, block.width / 6, block.height / 6);
  const width = Math.max(1, block.width - padding * 2);
  const sourceLines = Math.max(1, block.sourceText.split(/\r?\n/).length);
  const nominal = Math.max(
    1,
    Math.min(72, (block.height / sourceLines) * 0.78),
  );
  const minimum = Math.min(nominal, Math.max(9, nominal * 0.5));
  let fontSize = nominal;
  let lines = wrapTranslationText(
    block.translatedText,
    width,
    fontSize,
    measure,
  );
  while (
    lines.length * fontSize * 1.2 + padding * 2 >
      Math.max(block.height, maxHeight) &&
    fontSize > minimum
  ) {
    fontSize = Math.max(minimum, fontSize - 1);
    lines = wrapTranslationText(block.translatedText, width, fontSize, measure);
  }
  const lineHeight = fontSize * 1.2;
  const height = Math.min(
    Math.max(block.height, lines.length * lineHeight + padding * 2),
    Math.max(block.height, maxHeight),
  );
  const capacity = Math.max(1, Math.floor((height - padding * 2) / lineHeight));
  const overflow =
    lines.length > capacity ||
    lines.some((line) => {
      return measure(line, fontSize) > width;
    });
  const visible = lines.slice(0, capacity);
  if (overflow && visible.length) {
    const characters = Array.from(visible[visible.length - 1]);
    while (
      characters.length &&
      measure(`${characters.join("")}…`, fontSize) > width
    ) {
      characters.pop();
    }
    visible[visible.length - 1] = `${characters.join("")}…`;
  }
  return { fontSize, height, lineHeight, lines: visible, overflow, padding };
};

export const sampleTranslationBackground = (
  pixels: Uint8Array,
  width: number,
  height: number,
  block: Pick<ScreenshotTranslationBlock, "x" | "y" | "width" | "height">,
) => {
  const channels: number[][] = [[], [], []];
  const border: number[][] = [];
  const sample = (x: number, y: number) => {
    const column = Math.max(0, Math.min(width - 1, Math.round(x)));
    const row = Math.max(0, Math.min(height - 1, Math.round(y)));
    const index = (row * width + column) * 4;
    if (index + 3 >= pixels.length || pixels[index + 3] < 128) return;
    border.push(Array.from(pixels.subarray(index, index + 3)));
    for (let channel = 0; channel < 3; channel += 1) {
      channels[channel].push(pixels[index + channel]);
    }
  };
  for (let step = 0; step <= 12; step += 1) {
    const amount = step / 12;
    sample(block.x + block.width * amount, block.y - 2);
    sample(block.x + block.width * amount, block.y + block.height + 2);
    sample(block.x - 2, block.y + block.height * amount);
    sample(block.x + block.width + 2, block.y + block.height * amount);
  }
  const rgb = channels.map((values) => {
    values.sort((a, b) => {
      return a - b;
    });
    return values.length ? values[Math.floor(values.length / 2)] : 255;
  });
  const lightness = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
  const fallback = lightness > 145 ? "#111111" : "#F5F5F5";
  const distance = (color: readonly number[], other: readonly number[]) => {
    return Math.hypot(
      color[0] - other[0],
      color[1] - other[1],
      color[2] - other[2],
    );
  };
  const backgroundMatches = border.filter((color) => {
    return distance(color, rgb) < 48;
  }).length;
  // A textured border cannot reliably separate text from the image behind it.
  if (backgroundMatches < channels[0].length * 0.8) {
    return { background: `rgb(${rgb.join(", ")})`, foreground: fallback };
  }

  const candidates: number[][] = [];
  const buckets = new Map<string, number[][]>();
  const stride = Math.max(
    1,
    Math.ceil(Math.sqrt((block.width * block.height) / 16384)),
  );
  let sampled = 0;
  for (
    let y = Math.max(0, Math.ceil(block.y));
    y < Math.min(height, block.y + block.height);
    y += stride
  ) {
    for (
      let x = Math.max(0, Math.ceil(block.x));
      x < Math.min(width, block.x + block.width);
      x += stride
    ) {
      const index = (y * width + x) * 4;
      if (index + 3 >= pixels.length || pixels[index + 3] < 128) continue;
      sampled += 1;
      const color = Array.from(pixels.subarray(index, index + 3));
      if (distance(color, rgb) < 64) continue;
      candidates.push(color);
      const key = color
        .map((value) => {
          return Math.floor(value / 32);
        })
        .join(",");
      const bucket = buckets.get(key) ?? [];
      bucket.push(color);
      buckets.set(key, bucket);
    }
  }
  let dominant: number[][] = [];
  for (const bucket of buckets.values()) {
    if (bucket.length > dominant.length) dominant = bucket;
  }
  const core = dominant[0];
  const cluster = core
    ? candidates.filter((color) => {
        return distance(color, core) < 56;
      })
    : [];
  const confident =
    candidates.length >= Math.max(4, sampled * 0.005) &&
    candidates.length <= sampled * 0.6 &&
    cluster.length >= candidates.length * 0.65;
  // Ignore antialiased edges; use the dominant ink pixels, not their blend with the background.
  const ink = [0, 1, 2].map((channel) => {
    const values = cluster
      .map((color) => {
        return color[channel];
      })
      .sort((a, b) => {
        return a - b;
      });
    return values[Math.floor(values.length / 2)] ?? 0;
  });
  return {
    background: `rgb(${rgb.join(", ")})`,
    foreground: confident ? `rgb(${ink.join(", ")})` : fallback,
  };
};

export const createTranslationLayer = (
  area: PixelRect,
  blocks: readonly ScreenshotTranslationBlock[],
  pixels: Uint8Array,
): TranslationLayer => {
  const context = new OffscreenCanvas(1, 1).getContext("2d");
  if (!context) throw new Error("2d canvas context is unavailable");
  const measure: Measure = (text, fontSize) => {
    context.font = `${fontSize}px ${FONT}`;
    return context.measureText(text).width;
  };
  const valid = blocks.filter((block) => {
    return (
      [block.x, block.y, block.width, block.height].every(Number.isFinite) &&
      block.x >= 0 &&
      block.y >= 0 &&
      block.width > 0 &&
      block.height > 0 &&
      block.x + block.width <= area.width + 0.01 &&
      block.y + block.height <= area.height + 0.01 &&
      block.translatedText.trim().length > 0
    );
  });
  const painted = valid.map((block) => {
    let bottom = Math.min(area.height, block.y + block.height * 3);
    for (const other of valid) {
      if (
        other.id !== block.id &&
        other.y >= block.y + block.height - 0.01 &&
        other.x < block.x + block.width &&
        other.x + other.width > block.x
      ) {
        bottom = Math.min(bottom, other.y - 1);
      }
    }
    const layout = fitTranslationText(
      block,
      Math.max(block.height, bottom - block.y),
      measure,
    );
    return {
      ...layout,
      ...sampleTranslationBackground(pixels, area.width, area.height, block),
      id: block.id,
      rect: {
        height: Math.min(layout.height, area.height - block.y),
        width: Math.min(block.width, area.width - block.x),
        x: area.x + block.x,
        y: area.y + block.y,
      },
    };
  });
  return {
    area,
    blocks: painted,
    overflowCount: painted.filter((block) => {
      return block.overflow;
    }).length,
  };
};

/** Paints the same translation layer in the editor and every raster export. */
export const drawTranslation = (context: Canvas2D, layer: TranslationLayer) => {
  context.save();
  context.beginPath();
  context.rect(layer.area.x, layer.area.y, layer.area.width, layer.area.height);
  context.clip();
  context.globalAlpha = 1;
  context.globalCompositeOperation = "source-over";
  context.textAlign = "left";
  context.textBaseline = "top";
  for (const block of layer.blocks) {
    const { rect, padding, fontSize, lineHeight } = block;
    context.save();
    context.beginPath();
    context.rect(rect.x, rect.y, rect.width, rect.height);
    context.clip();
    context.fillStyle = block.background;
    context.fillRect(rect.x, rect.y, rect.width, rect.height);
    context.fillStyle = block.foreground;
    context.font = `${fontSize}px ${FONT}`;
    block.lines.forEach((line, index) => {
      context.fillText(
        line,
        rect.x + padding,
        rect.y + padding + index * lineHeight,
      );
    });
    context.restore();
  }
  context.restore();
};
