import type { DragEvent, FC, MouseEvent, PointerEvent, Ref } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { popupClipboardItemMenu, startDragClipboardItem } from "@/commands";
import AssetImage from "@/components/AssetImage";
import KeyHint from "@/components/KeyHint";
import type { ItemActionLabels } from "@/constants/itemActions";
import type { ClipboardAction, ClipboardItem } from "@/types/clipboard";
import type { ItemAction } from "@/types/settings";
import { cn } from "@/utils/cn";
import { isMac } from "@/utils/is";
import { resolveItemTone } from "../../itemTone";
import ClipboardQuickActions from "./ClipboardQuickActions";
import FilesCard from "./FilesCard";
import ImageCard from "./ImageCard";
import NoteContentSwitcher from "./NoteContentSwitcher";
import TextCard from "./TextCard";

interface ClipboardCardProps {
  item: ClipboardItem;
  isSelected?: boolean;

  hintKey?: string;

  onQuickPaste?: () => void;

  isLinkActive?: boolean;

  onOpenLink?: () => void;
  onPointerEnter?: (event: PointerEvent<HTMLDivElement>) => void;
  onPointerLeave?: () => void;
  onPointerMove?: (event: PointerEvent<HTMLDivElement>) => void;
  onMouseDown?: (event: MouseEvent<HTMLDivElement>) => void;
  onAuxClick?: (event: MouseEvent<HTMLDivElement>) => void;
  onDoubleClick?: (event: MouseEvent<HTMLDivElement>) => void;
  availableActions?: ClipboardAction[];
  quickActions?: ItemAction[];
  quickActionLabels?: ItemActionLabels;
  onQuickAction?: (action: ItemAction) => Promise<void> | void;
  showOriginalOnHover?: boolean;
  rootRef?: Ref<HTMLDivElement>;
}

/** Dispatch an item by kind and provide shared selection and context-menu behavior. */
const ClipboardCard: FC<ClipboardCardProps> = (props) => {
  const {
    item,
    isSelected,
    hintKey,
    onQuickPaste,
    isLinkActive,
    onOpenLink,
    onPointerEnter,
    onPointerLeave,
    onPointerMove,
    onMouseDown,
    onAuxClick,
    onDoubleClick,
    availableActions,
    quickActions = [],
    quickActionLabels,
    onQuickAction,
    showOriginalOnHover = true,
    rootRef,
  } = props;
  const { kind, sourceAppId, subKind, sourceAppIconPath, sourceAppName } = item;
  const { t } = useTranslation("clipboard");
  const [hovered, setHovered] = useState(false);
  const typeKey = subKind ?? kind;
  const typeLabel = t(`types.${typeKey}`);
  const tone = resolveItemTone(kind);
  const body = renderBody(item, isLinkActive, onOpenLink);
  const showSensitiveIndicator = item.isSensitive && item.kind === "text";
  const showStatusIndicators = item.isPinned || showSensitiveIndicator;
  const sourceAppIcon = sourceAppId ? (
    <AssetImage
      alt={sourceAppName}
      className="size-4"
      src={sourceAppIconPath}
    />
  ) : (
    <img
      alt="Ultra Clipboard"
      className="pointer-events-none size-4"
      src={isMac ? "/logo-mac.png" : "/logo.png"}
    />
  );

  const handleDragStart = async (event: DragEvent) => {
    event.preventDefault();

    await startDragClipboardItem(item.id);
  };

  const handleContextMenu = async (event: MouseEvent) => {
    event.preventDefault();

    const actions = availableActions ?? item.availableActions ?? [];
    const { isFavorite, isPinned, note } = item;

    if (actions.length === 0) return;

    await popupClipboardItemMenu(
      item.id,
      [...actions],
      item.groupId,
      isFavorite,
      isPinned,
      Boolean(note),
    );
  };

  const handlePointerEnter = (event: PointerEvent<HTMLDivElement>) => {
    setHovered(true);
    onPointerEnter?.(event);
  };

  const handlePointerLeave = () => {
    setHovered(false);
    onPointerLeave?.();
  };

  return (
    <div
      aria-selected={isSelected}
      className={cn(
        "relative flex flex-col gap-1 overflow-hidden rounded-2 border p-2 pl-3 transition-colors duration-150 ease-out motion-reduce:transition-none",
        isSelected ? tone.cardSelected : tone.card,
      )}
      draggable
      onAuxClick={onAuxClick}
      onContextMenu={handleContextMenu}
      onDoubleClick={onDoubleClick}
      onDragStart={handleDragStart}
      onMouseDown={onMouseDown}
      onPointerEnter={handlePointerEnter}
      onPointerLeave={handlePointerLeave}
      onPointerMove={onPointerMove}
      ref={rootRef}
      role="option"
      tabIndex={-1}
    >
      <span
        aria-hidden="true"
        className={cn("absolute inset-y-0 left-0 w-1.5", tone.stripe)}
      />
      <div className="flex items-center justify-between text-ant-secondary text-xs">
        <div className="flex min-w-0 items-center gap-1 overflow-hidden">
          {hintKey ? (
            <KeyHint hintKey={hintKey} onKeyPress={onQuickPaste}>
              {sourceAppIcon}
            </KeyHint>
          ) : (
            sourceAppIcon
          )}

          <span
            className={cn(
              "truncate rounded-full px-1.5 py-px font-medium text-[11px] leading-4",
              tone.pill,
            )}
          >
            {typeLabel}
          </span>
        </div>

        <ClipboardQuickActions
          item={item}
          labels={quickActionLabels}
          onQuickAction={onQuickAction}
          quickActions={quickActions}
          visible={hovered}
        />
      </div>

      {item.note ? (
        <NoteContentSwitcher
          note={item.note}
          showOriginal={showOriginalOnHover && hovered}
        >
          {body}
        </NoteContentSwitcher>
      ) : (
        body
      )}
      {showStatusIndicators
        ? renderStatusIndicators(item.isPinned, showSensitiveIndicator)
        : null}
    </div>
  );
};

/** Render the non-interactive status watermark in the card corner. */
function renderStatusIndicators(isPinned: boolean, isSensitive: boolean) {
  return (
    <div className="pointer-events-none absolute right-2 bottom-2 flex items-end gap-1 text-ant-quaternary">
      {isPinned ? (
        <i aria-hidden="true" className="i-ph:push-pin-bold size-5" />
      ) : null}
      {isSensitive ? (
        <i aria-hidden="true" className="i-lucide:key-round size-5" />
      ) : null}
    </div>
  );
}

const renderBody = (
  item: ClipboardItem,
  isLinkActive?: boolean,
  onOpenLink?: () => void,
) => {
  if (item.kind === "image") return <ImageCard {...item} />;

  if (item.kind === "files") return <FilesCard {...item} />;

  return (
    <TextCard {...item} isLinkActive={isLinkActive} onOpenLink={onOpenLink} />
  );
};

export default ClipboardCard;
