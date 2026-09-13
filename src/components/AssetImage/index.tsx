import { convertFileSrc } from "@tauri-apps/api/core";
import type { FC, ImgHTMLAttributes } from "react";
import { cn } from "@/utils/cn";

interface AssetImageProps
  extends Omit<ImgHTMLAttributes<HTMLImageElement>, "src"> {
  src?: string | null;
  protocol?: string;
}

const AssetImage: FC<AssetImageProps> = (props) => {
  const { alt, protocol, src, className, ...rest } = props;

  if (!src) return null;

  return (
    <img
      alt={alt}
      src={toAssetUrl(src, protocol)}
      {...rest}
      className={cn("pointer-events-none", className)}
    />
  );
};

const toAssetUrl = (filePath?: string | null, protocol?: string) => {
  if (!filePath) return "";

  if (!protocol) return convertFileSrc(filePath);

  return convertFileSrc(filePath, protocol);
};

export default AssetImage;
