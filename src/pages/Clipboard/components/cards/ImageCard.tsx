import type { CSSProperties, FC } from "react";
import { useSnapshot } from "valtio";
import AssetImage from "@/components/AssetImage";
import { settingsState } from "@/stores/settings";
import type { ClipboardItem } from "@/types/clipboard";

/** Load and render the backend-provided thumbnail for an image item. */
const ImageCard: FC<ClipboardItem> = (props) => {
  const { imageThumbnailPath } = props;
  const { clipboard } = useSnapshot(settingsState);
  const style: CSSProperties = {
    maxHeight: clipboard.display.imageMaxHeight,
  };

  return (
    <AssetImage className="self-start" src={imageThumbnailPath} style={style} />
  );
};

export default ImageCard;
