import { useMemoizedFn } from "ahooks";
import { Button, Dropdown } from "antd";
import type {
  FC,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
} from "react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ScreenshotExportAction } from "@/commands";
import Tooltip from "@/components/Tooltip";
import { cn } from "@/utils/cn";
import { isMac, isWin } from "@/utils/is";
import {
  type EditorTool,
  loadToolOrder,
  saveToolOrder,
  toolDefinition,
} from "../model/tools";
import ToolbarButton from "./ToolbarButton";
import ToolbarInfo from "./ToolbarInfo";

/** Where a tool's options panel should anchor, in viewport pixels. */
export interface ToolAnchorRect {
  left: number;
  bottom: number;
}

interface EditorToolbarProps {
  busyAction: ScreenshotExportAction | null;
  color: string | null;
  imageHeight: number;
  imageWidth: number;
  maximized: boolean;
  tool: EditorTool;
  zoom: number;
  /** Reported whenever the active tool's own button moves, so its options
   * panel can anchor right under it instead of a fixed corner. */
  onActiveToolRect: (rect: ToolAnchorRect | null) => void;
  onClose: () => void;
  onCopy: () => void;
  /** Copies the export to the clipboard, then closes the editor window. */
  onCopyAndClose: () => void;
  /** Closes the editor window without copying or saving anything. */
  onDiscardAndClose: () => void;
  onDragPointerDown: (event: ReactPointerEvent<HTMLElement>) => void;
  onDragPointerEnter: () => void;
  onMaximize: () => void;
  onMinimize: () => void;
  onMoveWindow: () => void;
  onPasteImage: () => void;
  onPin: () => void;
  /** Present only where the platform can recognize text. */
  onRecognizeText?: () => void;
  onSave: () => void;
  /** `openOptions` (a double-click) also opens the tool's options panel. */
  onToolChange: (tool: EditorTool, openOptions?: boolean) => void;
  onZoomToggle: () => void;
}

export const MOD_KEY = isMac ? "⌘" : "Ctrl+";
const DRAG_THRESHOLD_CSS = 3;
const REORDER_THRESHOLD_CSS = 4;
const PASTE_IMAGE_KEY = "paste-image";
const RECOGNIZE_TEXT_KEY = "recognize-text";

interface ReorderState {
  id: EditorTool;
  pointerId: number;
  startX: number;
  moved: boolean;
}

const toolTitle = (t: (key: string) => string, id: EditorTool) => {
  if (id === "select") return t("editor.select");
  if (id === "ruler") return t("editor.rulerHint");

  return t(`editor.tools.${id}`);
};

/** One-row toolbar that also acts as the window title bar, like the Shottr editor. */
const EditorToolbar: FC<EditorToolbarProps> = (props) => {
  const { t } = useTranslation("screenshot");
  const {
    busyAction,
    color,
    imageHeight,
    imageWidth,
    maximized,
    tool,
    zoom,
    onActiveToolRect,
    onClose,
    onCopy,
    onCopyAndClose,
    onDiscardAndClose,
    onDragPointerDown,
    onDragPointerEnter,
    onMaximize,
    onMinimize,
    onMoveWindow,
    onPasteImage,
    onPin,
    onRecognizeText,
    onSave,
    onToolChange,
    onZoomToggle,
  } = props;
  const pressRef = useRef<{ x: number; y: number } | null>(null);
  const reorderRef = useRef<ReorderState | null>(null);
  const toolRefs = useRef<Partial<Record<EditorTool, HTMLDivElement>>>({});
  const [toolOrder, setToolOrder] = useState<EditorTool[]>(loadToolOrder);
  const [draggingId, setDraggingId] = useState<EditorTool | null>(null);
  const busy = busyAction !== null;

  const reportActiveToolRect = useMemoizedFn(() => {
    const node = toolRefs.current[tool];
    if (!node) {
      onActiveToolRect(null);
      return;
    }

    const rect = node.getBoundingClientRect();
    onActiveToolRect({ bottom: rect.bottom, left: rect.left });
  });

  // Tracks the active tool's own button so its options panel can anchor under
  // it, following selection and drag-reorder; the resize listener covers the
  // window being resized or maximized while a panel is open.
  // biome-ignore lint/correctness/useExhaustiveDependencies: tool/toolOrder are the actual triggers; reportActiveToolRect is a stable useMemoizedFn reference.
  useEffect(() => {
    reportActiveToolRect();
  }, [tool, toolOrder, reportActiveToolRect]);

  useEffect(() => {
    window.addEventListener("resize", reportActiveToolRect);

    return () => {
      window.removeEventListener("resize", reportActiveToolRect);
    };
  }, [reportActiveToolRect]);

  const isTitleArea = (event: ReactMouseEvent<HTMLElement>) => {
    const target = event.target as HTMLElement;

    return (
      target === event.currentTarget || target.dataset.titleArea === "true"
    );
  };

  const handleTitleMouseDown = (event: ReactMouseEvent<HTMLElement>) => {
    if (event.button !== 0 || !isTitleArea(event)) return;

    pressRef.current = { x: event.clientX, y: event.clientY };
  };

  // The window moves only after the pointer travels, so double-click can maximize.
  const handleTitleMouseMove = (event: ReactMouseEvent<HTMLElement>) => {
    const press = pressRef.current;
    if (!press || (event.buttons & 1) === 0) {
      pressRef.current = null;
      return;
    }

    const distance = Math.hypot(
      event.clientX - press.x,
      event.clientY - press.y,
    );
    if (distance < DRAG_THRESHOLD_CSS) return;

    pressRef.current = null;
    onMoveWindow();
  };

  const handleTitleMouseUp = () => {
    pressRef.current = null;
  };

  const handleTitleDoubleClick = (event: ReactMouseEvent<HTMLElement>) => {
    if (!isWin || !isTitleArea(event)) return;

    onMaximize();
  };

  /** Drags a tool icon past a neighbor to swap places with it, live. */
  const handleToolPointerMove = (event: ReactPointerEvent<HTMLElement>) => {
    const dragging = reorderRef.current;
    if (!dragging || event.pointerId !== dragging.pointerId) return;

    if (!dragging.moved) {
      if (Math.abs(event.clientX - dragging.startX) < REORDER_THRESHOLD_CSS) {
        return;
      }
      dragging.moved = true;
      setDraggingId(dragging.id);
    }

    const target = document
      .elementFromPoint(event.clientX, event.clientY)
      ?.closest<HTMLElement>("[data-tool-id]");
    const overId = target?.dataset.toolId as EditorTool | undefined;
    if (!overId || overId === dragging.id) return;

    setToolOrder((current) => {
      const from = current.indexOf(dragging.id);
      const to = current.indexOf(overId);
      if (from === -1 || to === -1) return current;

      const next = [...current];
      next.splice(from, 1);
      next.splice(to, 0, dragging.id);

      return next;
    });
  };

  const finishReorder = () => {
    const dragging = reorderRef.current;
    reorderRef.current = null;
    setDraggingId(null);
    if (dragging?.moved) {
      setToolOrder((current) => {
        saveToolOrder(current);

        return current;
      });
    }
  };

  const handleToolPointerDown = (
    id: EditorTool,
    event: ReactPointerEvent<HTMLElement>,
  ) => {
    if (event.button !== 0) return;

    reorderRef.current = {
      id,
      moved: false,
      pointerId: event.pointerId,
      startX: event.clientX,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const handleToolPointerUp = (
    id: EditorTool,
    event: ReactPointerEvent<HTMLElement>,
  ) => {
    const dragging = reorderRef.current;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }

    const wasReorder = dragging?.id === id && dragging.moved;
    finishReorder();
    if (!wasReorder) onToolChange(id);
  };

  return (
    <header
      aria-label={t("editor.toolbar")}
      className={cn(
        // The editor is always dark (see App.tsx), so the container tone reads
        // as an elevated bar over the darker canvas regardless of what's captured.
        "flex h-13 shrink-0 select-none items-center gap-1 border-ant-border-secondary border-b bg-ant-container pr-1 pl-2 shadow-[0_1px_4px_rgba(0,0,0,0.35)]",
        { "pl-20": isMac },
      )}
      onDoubleClick={handleTitleDoubleClick}
      onMouseDown={handleTitleMouseDown}
      onMouseMove={handleTitleMouseMove}
      onMouseUp={handleTitleMouseUp}
      role="toolbar"
    >
      <ToolbarButton
        disabled={busy}
        icon="i-lucide:copy"
        loading={busyAction === "copy"}
        onClick={onCopy}
        shortcut={`${MOD_KEY}C`}
        title={t("editor.copy")}
      />
      <ToolbarButton
        disabled={busy}
        icon="i-lucide:save"
        loading={busyAction === "save"}
        onClick={onSave}
        shortcut={`${MOD_KEY}S`}
        title={t("editor.save")}
      />

      <span className="mx-1 h-5 w-px shrink-0 bg-ant-border-secondary" />

      <ToolbarButton
        disabled={busy}
        icon="i-lucide:pin"
        loading={busyAction === "pin"}
        onClick={onPin}
        title={t("editor.pin")}
      />

      <span className="mx-1 h-5 w-px shrink-0 bg-ant-border-secondary" />

      <ToolbarButton
        className="cursor-grab"
        icon="i-lucide:grip-vertical"
        onPointerDown={onDragPointerDown}
        onPointerEnter={onDragPointerEnter}
        title={t("editor.drag")}
      />

      <span className="mx-2 h-5 w-px shrink-0 bg-ant-border-secondary" />

      <div
        className="flex h-full min-w-0 items-center gap-1 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
        title={t("editor.reorderHint")}
      >
        {toolOrder.map((id) => {
          const definition = toolDefinition(id);

          return (
            <div
              className={cn("shrink-0", {
                "opacity-40": draggingId === id,
              })}
              data-tool-id={id}
              key={id}
              ref={(el) => {
                if (el) toolRefs.current[id] = el;
              }}
            >
              <ToolbarButton
                active={tool === id}
                icon={definition.icon}
                iconColor={definition.color}
                onDoubleClick={() => onToolChange(id, true)}
                onPointerDown={(event) => handleToolPointerDown(id, event)}
                onPointerMove={handleToolPointerMove}
                onPointerUp={(event) => handleToolPointerUp(id, event)}
                shortcut={definition.key}
                title={toolTitle(t, id)}
              />
            </div>
          );
        })}
      </div>

      <Dropdown
        menu={{
          items: [
            {
              extra: `${MOD_KEY}V`,
              icon: <span className="i-lucide:clipboard-paste text-base" />,
              key: PASTE_IMAGE_KEY,
              label: t("editor.pasteImage"),
            },
            ...(onRecognizeText
              ? [
                  {
                    extra: `${MOD_KEY}Shift+O`,
                    icon: <span className="i-lucide:scan-text text-base" />,
                    key: RECOGNIZE_TEXT_KEY,
                    label: t("editor.recognizeText"),
                  },
                ]
              : []),
          ],
          onClick: ({ key }) => {
            if (key === PASTE_IMAGE_KEY) {
              onPasteImage();
              return;
            }

            if (key === RECOGNIZE_TEXT_KEY) {
              onRecognizeText?.();
            }
          },
        }}
        placement="bottomLeft"
        trigger={["click"]}
      >
        <Button
          aria-label={t("editor.more")}
          className="h-9 shrink-0 gap-0.5 rounded-2 px-1.5"
          type="text"
        >
          <span className="i-lucide:ellipsis text-lg" />
          <span className="i-lucide:chevron-down text-xs" />
        </Button>
      </Dropdown>

      <div className="h-full min-w-6 flex-1" data-title-area="true" />

      {color && (
        <ToolbarInfo
          caption={t("editor.colorCaption")}
          leading={
            <span
              className="h-4 w-4 shrink-0 rounded-full border border-ant-border"
              style={{ backgroundColor: color }}
            />
          }
          value={color}
        />
      )}

      <span className="h-6 w-px shrink-0 bg-ant-border-secondary" />

      <ToolbarInfo
        caption={t("editor.imageSizeCaption")}
        value={`${imageWidth}×${imageHeight}`}
      />

      <span className="h-6 w-px shrink-0 bg-ant-border-secondary" />

      <ToolbarInfo
        caption={t("editor.zoomCaption")}
        onClick={onZoomToggle}
        value={`${Math.round(zoom * 100)}%`}
      />

      <ToolbarButton
        className="ml-2"
        danger
        disabled={busy}
        icon="i-lucide:trash-2"
        onClick={onDiscardAndClose}
        title={t("editor.discardAndClose")}
      />

      <Tooltip
        mouseEnterDelay={0.4}
        placement="bottom"
        title={`${t("editor.copyAndClose")} (${MOD_KEY}Enter)`}
      >
        <Button
          aria-label={t("editor.copyAndClose")}
          className="ml-1 h-9 shrink-0 gap-1.5 rounded-2 px-3 font-medium"
          disabled={busy}
          loading={busyAction === "copy"}
          onClick={onCopyAndClose}
          type="primary"
        >
          <span className="i-lucide:clipboard-check text-base" />
          {t("editor.copyAndClose")}
        </Button>
      </Tooltip>

      {isWin && (
        <div className="ml-1 flex shrink-0 items-center">
          <ToolbarButton
            icon="i-lucide:minus"
            onClick={onMinimize}
            title={t("editor.window.minimize")}
          />
          <ToolbarButton
            icon={maximized ? "i-lucide:copy" : "i-lucide:square"}
            onClick={onMaximize}
            title={t(
              maximized ? "editor.window.restore" : "editor.window.maximize",
            )}
          />
          <ToolbarButton
            danger
            icon="i-lucide:x"
            onClick={onClose}
            title={t("editor.window.close")}
          />
        </div>
      )}
    </header>
  );
};

export default EditorToolbar;
