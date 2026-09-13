import { inflateSync } from "node:zlib";

const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

export interface AlphaRaster {
  readonly alpha: Uint8Array;
  readonly height: number;
  readonly width: number;
}

const paethPredictor = (left: number, above: number, upperLeft: number) => {
  const prediction = left + above - upperLeft;
  const leftDistance = Math.abs(prediction - left);
  const aboveDistance = Math.abs(prediction - above);
  const upperLeftDistance = Math.abs(prediction - upperLeft);

  if (leftDistance <= aboveDistance && leftDistance <= upperLeftDistance) {
    return left;
  }
  if (aboveDistance <= upperLeftDistance) return above;

  return upperLeft;
};

const decodeRgbaPng = (png: Buffer): AlphaRaster => {
  if (!png.subarray(0, PNG_SIGNATURE.length).equals(PNG_SIGNATURE)) {
    throw new Error("ICO frame is not PNG encoded");
  }

  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  let interlaceMethod = 0;
  let offset = PNG_SIGNATURE.length;
  const imageDataChunks: Buffer[] = [];

  while (offset < png.length) {
    const chunkLength = png.readUInt32BE(offset);
    const chunkType = png.toString("ascii", offset + 4, offset + 8);
    const dataOffset = offset + 8;
    const nextOffset = dataOffset + chunkLength + 4;
    if (nextOffset > png.length) {
      throw new Error("PNG chunk exceeds frame data");
    }

    if (chunkType === "IHDR") {
      width = png.readUInt32BE(dataOffset);
      height = png.readUInt32BE(dataOffset + 4);
      bitDepth = png[dataOffset + 8] ?? 0;
      colorType = png[dataOffset + 9] ?? 0;
      interlaceMethod = png[dataOffset + 12] ?? 0;
    } else if (chunkType === "IDAT") {
      imageDataChunks.push(png.subarray(dataOffset, dataOffset + chunkLength));
    } else if (chunkType === "IEND") {
      break;
    }

    offset = nextOffset;
  }

  if (
    width === 0 ||
    height === 0 ||
    bitDepth !== 8 ||
    colorType !== 6 ||
    interlaceMethod !== 0
  ) {
    throw new Error("ICO PNG frames must be non-interlaced 8-bit RGBA images");
  }

  const bytesPerPixel = 4;
  const rowLength = width * bytesPerPixel;
  const filtered = inflateSync(Buffer.concat(imageDataChunks));
  if (filtered.length !== height * (rowLength + 1)) {
    throw new Error("PNG scanline data has an unexpected length");
  }

  const rgba = new Uint8Array(width * height * bytesPerPixel);
  let filteredOffset = 0;
  for (let row = 0; row < height; row += 1) {
    const filterType = filtered[filteredOffset] ?? -1;
    filteredOffset += 1;
    const rowOffset = row * rowLength;

    for (let column = 0; column < rowLength; column += 1) {
      const encoded = filtered[filteredOffset] ?? 0;
      filteredOffset += 1;
      const left =
        column >= bytesPerPixel
          ? (rgba[rowOffset + column - bytesPerPixel] ?? 0)
          : 0;
      const above = row > 0 ? (rgba[rowOffset + column - rowLength] ?? 0) : 0;
      const upperLeft =
        row > 0 && column >= bytesPerPixel
          ? (rgba[rowOffset + column - rowLength - bytesPerPixel] ?? 0)
          : 0;
      let predictor = 0;

      if (filterType === 1) predictor = left;
      else if (filterType === 2) predictor = above;
      else if (filterType === 3) predictor = Math.floor((left + above) / 2);
      else if (filterType === 4) {
        predictor = paethPredictor(left, above, upperLeft);
      } else if (filterType !== 0) {
        throw new Error(`Unsupported PNG filter type: ${filterType}`);
      }

      rgba[rowOffset + column] = (encoded + predictor) & 0xff;
    }
  }

  const alpha = new Uint8Array(width * height);
  for (let index = 0; index < alpha.length; index += 1) {
    alpha[index] = rgba[index * bytesPerPixel + 3] ?? 0;
  }

  return { alpha, height, width };
};

export const parseIcoAlphaRasters = (icon: Buffer) => {
  if (
    icon.length < 6 ||
    icon.readUInt16LE(0) !== 0 ||
    icon.readUInt16LE(2) !== 1
  ) {
    throw new Error("Invalid ICO header");
  }

  const frameCount = icon.readUInt16LE(4);
  const directoryEnd = 6 + frameCount * 16;
  if (directoryEnd > icon.length) throw new Error("ICO directory is truncated");

  const rasters = new Map<number, AlphaRaster>();
  for (let index = 0; index < frameCount; index += 1) {
    const entryOffset = 6 + index * 16;
    const width = icon[entryOffset] === 0 ? 256 : (icon[entryOffset] ?? 0);
    const height =
      icon[entryOffset + 1] === 0 ? 256 : (icon[entryOffset + 1] ?? 0);
    const byteLength = icon.readUInt32LE(entryOffset + 8);
    const imageOffset = icon.readUInt32LE(entryOffset + 12);
    const imageEnd = imageOffset + byteLength;

    if (width !== height) {
      throw new Error(`ICO frame is not square: ${width}x${height}`);
    }
    if (imageOffset < directoryEnd || imageEnd > icon.length) {
      throw new Error(`ICO ${width}px frame data is out of bounds`);
    }

    const raster = decodeRgbaPng(icon.subarray(imageOffset, imageEnd));
    if (raster.width !== width || raster.height !== height) {
      throw new Error(
        `ICO directory and PNG dimensions differ for ${width}px frame`,
      );
    }
    if (rasters.has(width)) {
      throw new Error(`ICO has duplicate ${width}px frames`);
    }

    rasters.set(width, raster);
  }

  return rasters;
};

export const resizeAlpha = (source: AlphaRaster, size: number): AlphaRaster => {
  if (source.width === size && source.height === size) return source;

  const alpha = new Uint8Array(size * size);
  for (let targetY = 0; targetY < size; targetY += 1) {
    const sourceY = Math.max(
      0,
      Math.min(
        source.height - 1,
        ((targetY + 0.5) * source.height) / size - 0.5,
      ),
    );
    const top = Math.floor(sourceY);
    const bottom = Math.min(source.height - 1, top + 1);
    const verticalWeight = sourceY - top;

    for (let targetX = 0; targetX < size; targetX += 1) {
      const sourceX = Math.max(
        0,
        Math.min(
          source.width - 1,
          ((targetX + 0.5) * source.width) / size - 0.5,
        ),
      );
      const left = Math.floor(sourceX);
      const right = Math.min(source.width - 1, left + 1);
      const horizontalWeight = sourceX - left;
      const topAlpha =
        (source.alpha[top * source.width + left] ?? 0) *
          (1 - horizontalWeight) +
        (source.alpha[top * source.width + right] ?? 0) * horizontalWeight;
      const bottomAlpha =
        (source.alpha[bottom * source.width + left] ?? 0) *
          (1 - horizontalWeight) +
        (source.alpha[bottom * source.width + right] ?? 0) * horizontalWeight;

      alpha[targetY * size + targetX] = Math.round(
        topAlpha * (1 - verticalWeight) + bottomAlpha * verticalWeight,
      );
    }
  }

  return { alpha, height: size, width: size };
};
