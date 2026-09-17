import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEventListener, useMemoizedFn, useMount, useUnmount } from "ahooks";
import { theme } from "antd";
import type { FC, PointerEvent as ReactPointerEvent } from "react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  closeScreenshotWindow,
  copyScreenshotColor,
  exportScreenshot,
  getScreenshotImage,
  getScreenshotImageInfo,
  notifyScreenshotWindowReady,
  readScreenshotClipboardImage,
  type ScreenshotExportAction,
  type ScreenshotImageInfo,
  startScreenshotDrag,
} from "@/commands";
import { TAURI_EVENT } from "@/constants/events";
import { useTauriListen } from "@/hooks/useTauriListen";
import { getMessageApi, getModalApi } from "@/utils/feedback";
import { isWin } from "@/utils/is";
import { log } from "@/utils/log";
import {
  intersectRects,
  isArrowKey,
  nudgeRect,
  type PixelPoint,
  type PixelRect,
} from "@/utils/pixelRect";
import EditorCanvas, { type ViewSize } from "./components/EditorCanvas";
import EditorToolbar, { type ToolAnchorRect } from "./components/EditorToolbar";
import TextEditor, { type TextEditState } from "./components/TextEditor";
import ToolOptions, { type OptionsSubject } from "./components/ToolOptions";
import {
  type TextEditRequest,
  useCanvasInteraction,
} from "./hooks/useCanvasInteraction";
import {
  addShape,
  type Backdrop,
  createDocument,
  defaultBackdrop,
  documentBounds,
  type EditorDocument,
  findShape,
  removeShape,
  replaceShape,
  setBackdrop,
} from "./model/document";
import {
  commitHistory,
  createHistory,
  type History,
  redoHistory,
  undoHistory,
} from "./model/history";
import { type PixelSpans, readPixelHex } from "./model/pixels";
import { applyStyleToShape, styleFromShape } from "./model/shapeStyle";
import { createShapeId, moveShape, type TextShape } from "./model/shapes";
import {
  type EditorTool,
  loadToolStyle,
  saveToolStyle,
  type ToolStyle,
  toolByCode,
} from "./model/tools";
import {
  centerViewport,
  clampViewport,
  fitViewport,
  toImagePoint,
  type Viewport,
  ZOOM_STEP,
  zoomViewportAt,
} from "./model/viewport";
import { renderScenePixels } from "./render/renderPixels";
import { measureTextBox, TEXT_LINE_HEIGHT, textPadding } from "./render/text";

interface LoadedImage {
  bitmap: ImageBitmap;
  info: ScreenshotImageInfo;
  pixels: Uint8ClampedArray;
}

interface CloseRequestPayload {
  label: string;
}

interface DragPreparation {
  document: EditorDocument;
  ready: Promise<boolean>;
  selection: PixelRect | null;
}

type WindowExportAction = Exclude<ScreenshotExportAction, "prepareDrag">;

const FIT_MARGIN_CSS = 24;
const VISIBLE_CONTENT_CSS = 64;
const PASTE_MAX_RATIO = 0.8;

/** Screenshot editor window opened automatically after every capture. */
const ScreenshotEditor: FC = () => {
  const { t } = useTranslation(["screenshot", "commands"]);
  const { token } = theme.useToken();
  const currentWindow = getCurrentWebviewWindow();
  const label = currentWindow.label;
  const [image, setImage] = useState<LoadedImage | null>(null);
  const [history, setHistory] = useState<History<EditorDocument> | null>(null);
  const [draft, setDraft] = useState<EditorDocument | null>(null);
  const [tool, setTool] = useState<EditorTool>("select");
  const [style, setStyle] = useState<ToolStyle>(loadToolStyle);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [textEdit, setTextEdit] = useState<TextEditState | null>(null);
  const [measure, setMeasure] = useState<PixelSpans | null>(null);
  const [selection, setSelection] = useState<PixelRect | null>(null);
  const [viewport, setViewport] = useState<Viewport | null>(null);
  const [viewSize, setViewSize] = useState<ViewSize | null>(null);
  const [fitMode, setFitMode] = useState(true);
  const [hoverColor, setHoverColor] = useState<string | null>(null);
  const [panHeld, setPanHeld] = useState(false);
  const [zoomHeld, setZoomHeld] = useState(false);
  const [busyAction, setBusyAction] = useState<ScreenshotExportAction | null>(
    null,
  );
  const [maximized, setMaximized] = useState(false);
  const [toolAnchor, setToolAnchor] = useState<ToolAnchorRect | null>(null);
  const [toolOptionsOpen, setToolOptionsOpen] = useState(false);
  const bitmapRef = useRef<ImageBitmap | null>(null);
  const exportedDocumentRef = useRef<EditorDocument | null>(null);
  const dragPreparationRef = useRef<DragPreparation | null>(null);
  const textEditRef = useRef<TextEditState | null>(null);
  const closingRef = useRef(false);
  const present = history?.present ?? null;
  const shown = draft ?? present;
  const scale = Math.max(1, image?.info.scaleFactor ?? 1);
  const checker = [token.colorBgLayout, token.colorFillSecondary] as const;
  const bounds = shown ? documentBounds(shown) : null;

  textEditRef.current = textEdit;

  useMount(async () => {
    try {
      const info = await getScreenshotImageInfo(label);
      const buffer = await getScreenshotImage(label);
      const pixels = new Uint8ClampedArray(buffer);
      const bitmap = await createImageBitmap(
        new ImageData(pixels, info.width, info.height),
      );
      const initial = createDocument({
        height: info.height,
        width: info.width,
        x: 0,
        y: 0,
      });

      bitmapRef.current = bitmap;
      exportedDocumentRef.current = initial;
      setImage({ bitmap, info, pixels });
      setHistory(createHistory(initial));
    } catch (error) {
      log.error("load screenshot editor image failed", error);
    }
  });

  useUnmount(() => {
    bitmapRef.current?.close();
  });

  const boundsKey = bounds
    ? `${bounds.x},${bounds.y},${bounds.width},${bounds.height}`
    : null;

  // biome-ignore lint/correctness/useExhaustiveDependencies: the fit follows bound values, not document identity.
  useEffect(() => {
    if (bounds === null || !viewSize || !fitMode) return;
    if (viewSize.width <= 0 || viewSize.height <= 0) return;

    setViewport(
      fitViewport(
        bounds,
        viewSize.width,
        viewSize.height,
        FIT_MARGIN_CSS * viewSize.ratio,
      ),
    );
  }, [boundsKey, fitMode, viewSize]);

  const commit = (next: EditorDocument) => {
    setDraft(null);
    setHistory((current) => {
      return current ? commitHistory(current, next) : current;
    });
  };

  /** Commits a change derived from the latest document, so queued edits never drop each other. */
  const update = (change: (document: EditorDocument) => EditorDocument) => {
    setDraft(null);
    setHistory((current) => {
      return current
        ? commitHistory(current, change(current.present))
        : current;
    });
  };

  const handleViewSizeChange = useMemoizedFn((size: ViewSize) => {
    setViewSize(size);
  });

  const handleViewportChange = useMemoizedFn((next: Viewport) => {
    if (!shown || !viewSize) return;

    setFitMode(false);
    setViewport(
      clampViewport(
        next,
        documentBounds(shown),
        viewSize.width,
        viewSize.height,
        VISIBLE_CONTENT_CSS * viewSize.ratio,
      ),
    );
  });

  const handleHoverPoint = useMemoizedFn((point: PixelPoint | null) => {
    if (!image || !point) {
      setHoverColor(null);
      return;
    }

    setHoverColor(
      readPixelHex(image.pixels, image.info.width, image.info.height, point),
    );
  });

  const handleFirstPaint = useMemoizedFn(async () => {
    try {
      await notifyScreenshotWindowReady(label);
    } catch (error) {
      log.error("reveal screenshot editor failed", error);
    }
  });

  const handleActiveToolRect = useMemoizedFn((rect: ToolAnchorRect | null) => {
    setToolAnchor(rect);
  });

  const startTextEdit = (request: TextEditRequest) => {
    const { point, shape } = request;

    if (shape) {
      setTextEdit({
        color: shape.color,
        fontSize: shape.fontSize,
        id: shape.id,
        style: shape.style,
        text: shape.text,
        x: shape.x,
        y: shape.y,
      });
      return;
    }

    const fontSize = style.textSize * scale;
    const padding = textPadding(fontSize, style.textStyle);

    setTextEdit({
      color: style.color,
      fontSize,
      id: null,
      style: style.textStyle,
      text: "",
      x: point.x - padding.x,
      y: point.y - (fontSize * TEXT_LINE_HEIGHT) / 2 - padding.y,
    });
  };

  const commitTextEdit = () => {
    const edit = textEditRef.current;
    if (!edit || !present) return;

    textEditRef.current = null;
    setTextEdit(null);

    const text = edit.text.replace(/\s+$/u, "");
    const editedId = edit.id;

    if (text.trim() === "") {
      if (editedId) update((document) => removeShape(document, editedId));
      setSelectedId(null);
      return;
    }

    const shape: TextShape = {
      ...measureTextBox(text, edit.fontSize, edit.style),
      color: edit.color,
      fontSize: edit.fontSize,
      id: editedId ?? createShapeId(),
      kind: "text",
      style: edit.style,
      text,
      x: edit.x,
      y: edit.y,
    };

    update((document) => {
      return findShape(document, shape.id)
        ? replaceShape(document, shape)
        : addShape(document, shape);
    });
    setSelectedId(shape.id);
  };

  const interaction = useCanvasInteraction({
    commit,
    document: present ?? createDocument({ height: 1, width: 1, x: 0, y: 0 }),
    image: {
      height: image?.info.height ?? 0,
      pixels: image?.pixels ?? new Uint8ClampedArray(),
      width: image?.info.width ?? 0,
    },
    onEditText: startTextEdit,
    onMeasure: setMeasure,
    onSelectionChange: setSelection,
    onSelectShape: setSelectedId,
    scale,
    selectedId,
    selection,
    setDraft,
    style,
    tool,
  });

  const renderAndExport = async (action: ScreenshotExportAction) => {
    if (!image || !present) return null;

    const area = selection ?? documentBounds(present);
    const pixels = renderScenePixels(
      { bitmap: image.bitmap, document: present },
      area,
    );

    return exportScreenshot({ action, area, label, pixels });
  };

  const runExport = async (action: WindowExportAction) => {
    if (busyAction || !present) return;

    setBusyAction(action);

    try {
      const result = await renderAndExport(action);
      if (!result) return;

      if (action === "ocr") {
        const text = result.text ?? "";
        if (text) {
          getMessageApi().success(
            t("editor.ocrCopied", { count: Array.from(text).length }),
          );
        } else {
          getMessageApi().warning(t("editor.ocrEmpty"));
        }
        return;
      }

      exportedDocumentRef.current = present;

      if (action === "copy") {
        getMessageApi().success(t("commands:messages.copied"));
      }

      if (action === "save" && result.savedPath) {
        getMessageApi().success(
          t("editor.saved", { name: fileNameOf(result.savedPath) }),
        );
      }
    } catch (error) {
      log.error(`screenshot ${action} failed`, error);
    } finally {
      setBusyAction(null);
    }
  };

  const prepareDrag = () => {
    if (!present) return null;

    const current = dragPreparationRef.current;
    if (current?.document === present && current.selection === selection) {
      return current.ready;
    }

    const ready = (async () => {
      try {
        return (await renderAndExport("prepareDrag")) !== null;
      } catch (error) {
        log.error("prepare screenshot drag failed", error);
        dragPreparationRef.current = null;

        return false;
      }
    })();

    dragPreparationRef.current = { document: present, ready, selection };

    return ready;
  };

  const handleDragPointerEnter = () => {
    void prepareDrag();
  };

  const handleDragPointerDown = async (
    event: ReactPointerEvent<HTMLElement>,
  ) => {
    if (event.button !== 0) return;

    event.preventDefault();
    const ready = await prepareDrag();
    if (!ready) return;

    try {
      await startScreenshotDrag(label);
      exportedDocumentRef.current = present;
    } catch (error) {
      log.error("start screenshot drag failed", error);
    }
  };

  const copyHoverColor = async () => {
    if (!hoverColor) return;

    try {
      await copyScreenshotColor(hoverColor);
      getMessageApi().success(t("editor.colorCopied", { color: hoverColor }));
    } catch (error) {
      log.error("copy screenshot color failed", error);
    }
  };

  const pasteImage = async () => {
    if (!present || !viewport || !viewSize) return;

    try {
      const buffer = await readScreenshotClipboardImage();
      if (buffer.byteLength === 0) {
        getMessageApi().warning(t("editor.pasteEmpty"));
        return;
      }

      const bitmap = await createImageBitmap(
        new Blob([buffer], { type: "image/png" }),
      );
      const { crop } = present;
      const ratio = Math.min(
        1,
        (crop.width * PASTE_MAX_RATIO) / bitmap.width,
        (crop.height * PASTE_MAX_RATIO) / bitmap.height,
      );
      const width = Math.max(1, Math.round(bitmap.width * ratio));
      const height = Math.max(1, Math.round(bitmap.height * ratio));
      const center = toImagePoint(
        viewport,
        viewSize.width / 2,
        viewSize.height / 2,
      );
      const x = Math.round(
        Math.min(
          Math.max(center.x - width / 2, crop.x),
          crop.x + crop.width - width,
        ),
      );
      const y = Math.round(
        Math.min(
          Math.max(center.y - height / 2, crop.y),
          crop.y + crop.height - height,
        ),
      );
      const id = createShapeId();

      update((document) => {
        return addShape(document, {
          bitmap,
          height,
          id,
          kind: "image",
          width,
          x,
          y,
        });
      });
      setTool("select");
      setSelection(null);
      setSelectedId(id);
    } catch (error) {
      log.error("paste image into screenshot failed", error);
    }
  };

  /** A single click just picks the tool, ready to draw with its last-used
   * settings; `openOptions` (a double-click) also opens its options panel. */
  const changeTool = (next: EditorTool, openOptions = false) => {
    commitTextEdit();
    setTool(next);
    setSelectedId(null);
    setMeasure(null);
    if (next !== "select") setSelection(null);
    setToolOptionsOpen(openOptions);
  };

  const fit = () => {
    setFitMode(true);
  };

  const zoomBy = (factor: number) => {
    if (!viewport || !viewSize) return;

    handleViewportChange(
      zoomViewportAt(
        viewport,
        viewport.zoom * factor,
        viewSize.width / 2,
        viewSize.height / 2,
      ),
    );
  };

  const showActualSize = () => {
    if (!shown || !viewSize) return;

    handleViewportChange(
      centerViewport(documentBounds(shown), viewSize.width, viewSize.height, 1),
    );
  };

  const toggleZoom = () => {
    if (fitMode) {
      showActualSize();
      return;
    }

    fit();
  };

  const undo = () => {
    setSelection(null);
    setSelectedId(null);
    setDraft(null);
    setHistory((current) => {
      return current ? undoHistory(current) : current;
    });
  };

  const redo = () => {
    setSelection(null);
    setSelectedId(null);
    setDraft(null);
    setHistory((current) => {
      return current ? redoHistory(current) : current;
    });
  };

  const cropToSelection = () => {
    if (!selection || !present) return;

    const cropped = intersectRects(selection, present.crop);

    setSelection(null);
    if (!cropped) return;

    setFitMode(true);
    update((document) => ({ ...document, crop: cropped }));
  };

  const deleteSelectedShape = () => {
    if (!selectedId) return;

    const id = selectedId;
    update((document) => removeShape(document, id));
    setSelectedId(null);
  };

  const handleStyleChange = (patch: Partial<ToolStyle>, final: boolean) => {
    const nextStyle = { ...style, ...patch };
    setStyle(nextStyle);
    if (final) saveToolStyle(nextStyle);

    const edit = textEditRef.current;
    if (edit) {
      const nextEdit = {
        ...edit,
        color: patch.color ?? edit.color,
        fontSize:
          patch.textSize === undefined ? edit.fontSize : patch.textSize * scale,
        style: patch.textStyle ?? edit.style,
      };
      textEditRef.current = nextEdit;
      setTextEdit(nextEdit);
      return;
    }

    const shape = present ? findShape(present, selectedId) : null;
    if (!present || !shape || !image) return;

    const next = replaceShape(
      present,
      applyStyleToShape(
        shape,
        patch,
        scale,
        {
          height: image.info.height,
          pixels: image.pixels,
          width: image.info.width,
        },
        measureTextBox,
      ),
    );

    if (final) commit(next);
    else setDraft(next);
  };

  const handleBackdropChange = (backdrop: Backdrop, final: boolean) => {
    if (!present) return;

    const next = setBackdrop(present, backdrop);
    if (final) commit(next);
    else setDraft(next);
  };

  const handleBackdropToggle = (enabled: boolean) => {
    if (!present) return;

    setFitMode(true);
    update((document) => {
      return setBackdrop(
        document,
        enabled ? defaultBackdrop(document.crop, scale) : null,
      );
    });
  };

  const closeWindow = async () => {
    closingRef.current = true;

    try {
      await closeScreenshotWindow(label);
    } catch (error) {
      closingRef.current = false;
      log.error("close screenshot editor failed", error);
    }
  };

  const requestClose = () => {
    if (closingRef.current) return;

    commitTextEdit();

    if (!present || exportedDocumentRef.current === present) {
      void closeWindow();
      return;
    }

    getModalApi().confirm({
      cancelText: t("editor.closeConfirm.cancel"),
      content: t("editor.closeConfirm.content"),
      okText: t("editor.closeConfirm.ok"),
      onOk: closeWindow,
      title: t("editor.closeConfirm.title"),
    });
  };

  useTauriListen<CloseRequestPayload>(
    TAURI_EVENT.SCREENSHOT_EDITOR_CLOSE_REQUESTED,
    (event) => {
      if (event.payload.label !== label) return;

      requestClose();
    },
  );

  const moveWindow = async () => {
    try {
      await currentWindow.startDragging();
    } catch (error) {
      log.error("move screenshot editor window failed", error);
    }
  };

  const minimizeWindow = async () => {
    try {
      await currentWindow.minimize();
    } catch (error) {
      log.error("minimize screenshot editor failed", error);
    }
  };

  const toggleMaximizeWindow = async () => {
    try {
      await currentWindow.toggleMaximize();
      setMaximized(await currentWindow.isMaximized());
    } catch (error) {
      log.error("toggle screenshot editor maximize failed", error);
    }
  };

  const nudgeSelectedShape = (key: string, step: number) => {
    const shape = present ? findShape(present, selectedId) : null;
    if (!present || !shape) return false;

    const dx = key === "ArrowLeft" ? -step : key === "ArrowRight" ? step : 0;
    const dy = key === "ArrowUp" ? -step : key === "ArrowDown" ? step : 0;
    commit(replaceShape(present, moveShape(shape, dx, dy)));

    return true;
  };

  const handleModifiedKey = (event: KeyboardEvent, crop: PixelRect) => {
    const run = (action: () => void) => {
      event.preventDefault();
      action();
    };
    const key = event.key;

    if (selection && isArrowKey(key)) {
      run(() => {
        setSelection(
          nudgeRect(selection, key, event.shiftKey ? 10 : 1, true, crop),
        );
      });
      return;
    }

    switch (event.code) {
      case "KeyC":
        run(() => {
          void runExport("copy");
        });
        return;
      case "KeyS":
        run(() => {
          void runExport("save");
        });
        return;
      case "KeyV":
        run(() => {
          void pasteImage();
        });
        return;
      case "KeyZ":
        run(event.shiftKey ? redo : undo);
        return;
      case "KeyY":
        run(redo);
        return;
      case "KeyW":
        run(requestClose);
        return;
      case "Enter":
        run(() => {
          void handleCopyAndClose();
        });
        return;
      case "KeyO":
        if (event.shiftKey && isWin) {
          run(() => {
            void runExport("ocr");
          });
        }
        return;
      case "Digit0":
      case "Numpad0":
        run(fit);
        return;
      case "Digit1":
      case "Numpad1":
        run(showActualSize);
        return;
      case "Equal":
      case "NumpadAdd":
        run(() => {
          zoomBy(ZOOM_STEP);
        });
        return;
      case "Minus":
      case "NumpadSubtract":
        run(() => {
          zoomBy(1 / ZOOM_STEP);
        });
        return;
    }
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    if (!present || textEditRef.current) return;

    const target = event.target;
    if (
      target instanceof HTMLElement &&
      (target.closest("input, textarea, [contenteditable='true']") ||
        target.closest("[data-editor-options]"))
    ) {
      return;
    }

    const key = event.key;

    if (key === "Tab") {
      event.preventDefault();
      void copyHoverColor();
      return;
    }

    if (event.code === "Space") {
      event.preventDefault();
      setPanHeld(true);
      return;
    }

    if (event.ctrlKey || event.metaKey) {
      handleModifiedKey(event, present.crop);
      return;
    }

    if (event.altKey) return;

    if (event.code === "KeyZ") {
      setZoomHeld(true);
      return;
    }

    if (key === "Delete" || key === "Backspace") {
      if (selectedId) {
        event.preventDefault();
        deleteSelectedShape();
      }
      return;
    }

    if (key === "Enter") {
      event.preventDefault();
      cropToSelection();
      return;
    }

    if (key === "Escape") {
      if (selectedId) setSelectedId(null);
      else if (selection) setSelection(null);
      else if (tool !== "select") changeTool("select");
      return;
    }

    if (isArrowKey(key)) {
      const step = event.shiftKey ? 10 : 1;

      if (nudgeSelectedShape(key, step)) {
        event.preventDefault();
        return;
      }

      if (selection) {
        event.preventDefault();
        setSelection(nudgeRect(selection, key, step, false, present.crop));
      }
      return;
    }

    if (event.repeat) return;

    const definition = toolByCode(event.code);
    if (definition) {
      event.preventDefault();
      changeTool(definition.id);
    }
  };

  const handleKeyUp = (event: KeyboardEvent) => {
    if (event.code === "Space") setPanHeld(false);
    if (event.code === "KeyZ") setZoomHeld(false);
  };

  const handleWindowBlur = () => {
    setPanHeld(false);
    setZoomHeld(false);
  };

  useEventListener("keydown", handleKeyDown);
  useEventListener("keyup", handleKeyUp);
  useEventListener("blur", handleWindowBlur);

  if (!image || !present || !shown) {
    return <div className="h-screen bg-ant-layout" />;
  }

  const outputArea = selection ?? documentBounds(present);
  const selectedShape = textEdit ? null : findShape(shown, selectedId);
  const subject: OptionsSubject | null = textEdit
    ? "text"
    : selectedShape
      ? selectedShape.kind
      : tool !== "select" && toolOptionsOpen
        ? tool
        : null;
  const optionsStyle = textEdit
    ? {
        ...style,
        color: textEdit.color,
        textSize: textEdit.fontSize / scale,
        textStyle: textEdit.style,
      }
    : selectedShape
      ? styleFromShape(style, selectedShape, scale)
      : style;

  const handleCopy = () => {
    void runExport("copy");
  };

  const handleCopyAndClose = async () => {
    await runExport("copy");
    if (exportedDocumentRef.current === present) {
      await closeWindow();
    }
  };

  const handleDiscardAndClose = () => {
    void closeWindow();
  };

  const handleSave = () => {
    void runExport("save");
  };

  const handlePin = () => {
    void runExport("pin");
  };

  const handlePasteImage = () => {
    void pasteImage();
  };

  const handleRecognizeText = () => {
    void runExport("ocr");
  };

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-ant-layout">
      <EditorToolbar
        busyAction={busyAction}
        color={hoverColor}
        imageHeight={outputArea.height}
        imageWidth={outputArea.width}
        maximized={maximized}
        onActiveToolRect={handleActiveToolRect}
        onClose={requestClose}
        onCopy={handleCopy}
        onCopyAndClose={handleCopyAndClose}
        onDiscardAndClose={handleDiscardAndClose}
        onDragPointerDown={handleDragPointerDown}
        onDragPointerEnter={handleDragPointerEnter}
        onMaximize={toggleMaximizeWindow}
        onMinimize={minimizeWindow}
        onMoveWindow={moveWindow}
        onPasteImage={handlePasteImage}
        onPin={handlePin}
        onRecognizeText={isWin ? handleRecognizeText : undefined}
        onSave={handleSave}
        onToolChange={changeTool}
        onZoomToggle={toggleZoom}
        tool={tool}
        zoom={viewport?.zoom ?? 1}
      />

      <EditorCanvas
        checker={checker}
        interaction={interaction}
        measure={measure}
        onFirstPaint={handleFirstPaint}
        onHoverPoint={handleHoverPoint}
        onViewportChange={handleViewportChange}
        onViewSizeChange={handleViewSizeChange}
        panHeld={panHeld}
        scene={{
          bitmap: image.bitmap,
          document: shown,
          hiddenShapeId: textEdit?.id ?? null,
        }}
        selectedShape={selectedShape}
        selection={selection}
        viewport={viewport}
        zoomHeld={zoomHeld}
      >
        {subject && (
          <ToolOptions
            anchor={toolAnchor}
            backdrop={shown.backdrop}
            backdropLimit={Math.round(
              Math.max(
                64 * scale,
                Math.max(present.crop.width, present.crop.height) * 0.25,
              ),
            )}
            hasSelection={selectedShape !== null}
            onBackdropChange={handleBackdropChange}
            onBackdropToggle={handleBackdropToggle}
            onDelete={deleteSelectedShape}
            onStyleChange={handleStyleChange}
            style={optionsStyle}
            subject={subject}
          />
        )}

        {textEdit && viewport && viewSize && (
          <TextEditor
            edit={textEdit}
            onChange={(text) => {
              const next = { ...textEdit, text };
              textEditRef.current = next;
              setTextEdit(next);
            }}
            onCommit={commitTextEdit}
            ratio={viewSize.ratio}
            viewport={viewport}
          />
        )}
      </EditorCanvas>
    </div>
  );
};

function fileNameOf(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

export default ScreenshotEditor;
