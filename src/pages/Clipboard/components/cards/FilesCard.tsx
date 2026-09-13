import type { FC } from "react";
import { useSnapshot } from "valtio";
import AssetImage from "@/components/AssetImage";
import Highlight from "@/components/Highlight";
import { clipboardViewState } from "@/stores/clipboardView";
import type { ClipboardItem, FileEntry } from "@/types/clipboard";
import { cn } from "@/utils/cn";
import ImageCard from "./ImageCard";

/** Render preprocessed file entries as an image preview or file list. */
const FilesCard: FC<ClipboardItem> = (props) => {
  const entries = props.fileEntries ?? [];
  const { keyword } = useSnapshot(clipboardViewState);

  if (props.filesPreviewKind === "imagePreview") {
    const [first] = entries;
    return (
      <div className="flex flex-col gap-1">
        <ImageCard {...props} imageThumbnailPath={first?.path} />
        {first ? <FilePathLine keyword={keyword} path={first.path} /> : null}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1">
      {entries.map((entry) => {
        return <FileRow entry={entry} key={entry.path} keyword={keyword} />;
      })}
    </div>
  );
};

interface FileRowProps {
  entry: FileEntry;
  keyword: string;
}

const FileRow: FC<FileRowProps> = (props) => {
  const { entry, keyword } = props;

  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      <div className="flex items-center gap-1 truncate" title={entry.path}>
        <AssetImage className="size-5" src={entry.iconPath} />

        <Highlight
          className={cn("truncate", { "line-through": !entry.exists })}
          keyword={keyword}
          text={entry.name}
        />
      </div>
      <FilePathLine keyword={keyword} path={entry.path} />
    </div>
  );
};

const FilePathLine: FC<{ keyword: string; path: string }> = (props) => {
  return (
    <span className="block min-w-0 truncate" title={props.path}>
      <Highlight
        className="text-[11px] text-ant-tertiary leading-4"
        keyword={props.keyword}
        text={props.path}
      />
    </span>
  );
};

export default FilesCard;
