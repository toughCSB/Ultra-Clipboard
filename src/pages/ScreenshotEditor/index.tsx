import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEventListener, useMemoizedFn, useMount, useUnmount } from "ahooks";
import { theme } from "antd";
import type { FC, PointerEvent as ReactPointerEvent } from "react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  closeScreenshotWindow,
  copyScreenshotColor,
  copyScreenshotText,
  exportScreenshot,
  getScreenshotImage,
  getScreenshotImageInfo,
  getScreenshotTranslationStatus,
  notifyScreenshotWindowReady,
  readScreenshotClipboardImage,
  removeScreenshotTranslationKey,
  type ScreenshotExportAction,
  type ScreenshotImageInfo,
  type ScreenshotTextRegion,
  type ScreenshotTranslationStatus,
  setScreenshotTranslationKey,
  startScreenshotDrag,
} from "@/commands";
import { TAURI_EVENT } from "@/constants/events";
import { useTauriListen } from "@/hooks/useTauriListen";
import { getMessageApi, getModalApi } from "@/utils/feedback";
import { log } from "@/utils/log";
import {
  intersectRects,
  nudgeRect,
  type PixelPoint,
  type PixelRect,
} from "@/utils/pixelRect";
import EditorCanvas, { type ViewSize } from "./components/EditorCanvas";
import EditorToolbar, { type ToolAnchorRect } from "./components/EditorToolbar";
import ImageTextSelection, {
  selectedImageText,
} from "./components/ImageTextSelection";
import TextEditor, { type TextEditState } from "./components/TextEditor";
import ToolOptions, { type OptionsSubject } from "./components/ToolOptions";
import TranslationControls from "./components/TranslationControls";
import TranslationKeyModal from "./components/TranslationKeyModal";
import {
  type TextEditRequest,
  useCanvasInteraction,
} from "./hooks/useCanvasInteraction";
import { useTranslationSession } from "./hooks/useTranslationSession";
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
import { createDragGesture } from "./model/dragGesture";
import {
  createDragPreparationCache,
  type DragIdentity,
  sameDragIdentity,
} from "./model/dragPreparation";
import {
  createExportState,
  hasUnexportedChanges,
  recordExport,
} from "./model/exportState";
import {
  commitHistory,
  createHistory,
  type History,
  redoHistory,
  undoHistory,
} from "./model/history";
import {
  type EditorCommand,
  type KeyMode,
  type KeyTarget,
  resolveKey,
} from "./model/keymap";
import { type PixelSpans, readPixelHex } from "./model/pixels";
import { applyStyleToShape, styleFromShape } from "./model/shapeStyle";
import { createShapeId, moveShape, type TextShape } from "./model/shapes";
import { validTextRegions } from "./model/textSelection";
import {
  type EditorTool,
  loadToolStyle,
  saveToolStyle,
  type ToolStyle,
} from "./model/tools";
import {
  activeLayer,
  isAwaitingKey,
  isLoading,
  translatedText,
} from "./model/translationSession";
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

interface TextSelectionMode {
  area: PixelRect;
  document: EditorDocument;
  loading: boolean;
  regions: ScreenshotTextRegion[];
}

type WindowExportAction = Exclude<
  ScreenshotExportAction,
  "prepareDrag" | "selectText" | "translate"
>;

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
  const [textSelectionMode, setTextSelectionMode] =
    useState<TextSelectionMode | null>(null);
  const [selectedImageTextValue, setSelectedImageTextValue] = useState("");
  const [translationKeyModalOpen, setTranslationKeyModalOpen] = useState(false);
  const [translationKeyConfigured, setTranslationKeyConfigured] =
    useState(false);
  const [translationKeyModel, setTranslationKeyModel] = useState<string | null>(
    null,
  );
  const [translationKeySource, setTranslationKeySource] =
    useState<ScreenshotTranslationStatus["source"]>(null);
  const [translationKeyBusy, setTranslationKeyBusy] = useState(false);
  const bitmapRef = useRef<ImageBitmap | null>(null);
  const exportStateRef = useRef(createExportState<EditorDocument>(null));
  const dragPreparationRef = useRef(createDragPreparationCache());
  const dragGestureRef = useRef(createDragGesture());
  const dragDocumentRef = useRef<EditorDocument | null>(null);
  const dragSelectionRef = useRef<PixelRect | null>(null);
  const previousDragContextRef = useRef<{
    document: EditorDocument | null;
    selection: PixelRect | null;
  }>({ document: null, selection: null });
  const textEditRef = useRef<TextEditState | null>(null);
  const closingRef = useRef(false);
  const textRequestGenerationRef = useRef(0);
  const textSelectionModeRef = useRef<TextSelectionMode | null>(null);
  const textOverlayRef = useRef<HTMLDivElement>(null);
  const selectedImageTextRef = useRef("");
  const pendingTextStartRef = useRef(false);
  const loadStartedAtRef = useRef(performance.now());
  const present = history?.present ?? null;
  const presentRef = useRef(present);
  presentRef.current = present;
  const shown = draft ?? present;
  const applyTranslationKeyStatus = (status: ScreenshotTranslationStatus) => {
    setTranslationKeyConfigured(status.configured);
    setTranslationKeyModel(status.model);
    setTranslationKeySource(status.source);
  };

  const translation = useTranslationSession({
    label,
    onBusyChange: (busy) => {
      setBusyAction((current) => {
        if (busy) return "translate";
        return current === "translate" ? null : current;
      });
    },
    onKeyStatus: applyTranslationKeyStatus,
    onLayerChange: () => {
      dragPreparationRef.current.invalidate();
    },
    onNeedsKey: () => {
      setTranslationKeyModalOpen(true);
    },
    readEditor: () => {
      return {
        closing: closingRef.current,
        document: presentRef.current,
        selection: dragSelectionRef.current,
      };
    },
    renderPixels: (document, area) => {
      if (!image) return null;

      return renderScenePixels({ bitmap: image.bitmap, document }, area);
    },
  });
  const translationSession = translation.session;
  const activeTranslation = activeLayer(translationSession);
  dragDocumentRef.current = present;
  dragSelectionRef.current = selection;
  const scale = Math.max(1, image?.info.scaleFactor ?? 1);
  const checker = [token.colorBgLayout, token.colorFillSecondary] as const;
  const bounds = shown ? documentBounds(shown) : null;

  textEditRef.current = textEdit;

  useMount(async () => {
    try {
      const info = await getScreenshotImageInfo(label);
      const infoMs = performance.now() - loadStartedAtRef.current;
      const buffer = await getScreenshotImage(label);
      const pixelsMs = performance.now() - loadStartedAtRef.current - infoMs;
      const pixels = new Uint8ClampedArray(buffer);
      const bitmap = await createImageBitmap(
        new ImageData(pixels, info.width, info.height),
      );
      const bitmapMs =
        performance.now() - loadStartedAtRef.current - infoMs - pixelsMs;
      log.info("screenshot editor image ready", {
        bitmapMs: Math.round(bitmapMs),
        infoMs: Math.round(infoMs),
        label,
        pixelsMs: Math.round(pixelsMs),
      });
      const initial = createDocument({
        height: info.height,
        width: info.width,
        x: 0,
        y: 0,
      });

      bitmapRef.current = bitmap;
      exportStateRef.current = createExportState(initial);
      setImage({ bitmap, info, pixels });
      setHistory(createHistory(initial));
    } catch (error) {
      log.error("load screenshot editor image failed", error);
    }
  });

  useUnmount(() => {
    textRequestGenerationRef.current += 1;
    dragGestureRef.current.cancel();
    bitmapRef.current?.close();
  });

  useEffect(() => {
    const previous = previousDragContextRef.current;
    if (previous.document !== present || previous.selection !== selection) {
      dragGestureRef.current.cancel();
      previousDragContextRef.current = { document: present, selection };
    }
  }, [present, selection]);

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

  const leaveTextSelection = useMemoizedFn(() => {
    if (
      !textSelectionModeRef.current &&
      !pendingTextStartRef.current &&
      busyAction !== "selectText"
    ) {
      return;
    }

    textRequestGenerationRef.current += 1;
    pendingTextStartRef.current = false;
    textSelectionModeRef.current = null;
    selectedImageTextRef.current = "";
    window.getSelection()?.removeAllRanges();
    setSelectedImageTextValue("");
    setTextSelectionMode(null);
    setBusyAction((current) => {
      return current === "selectText" ? null : current;
    });
  });

  const commit = (next: EditorDocument) => {
    leaveTextSelection();
    translation.leave();
    setDraft(null);
    setHistory((current) => {
      return current ? commitHistory(current, next) : current;
    });
  };

  /** Commits a change derived from the latest document, so queued edits never drop each other. */
  const update = (change: (document: EditorDocument) => EditorDocument) => {
    leaveTextSelection();
    translation.leave();
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
    log.info("screenshot editor painted", {
      label,
      totalMs: Math.round(performance.now() - loadStartedAtRef.current),
    });
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

    const document = present;
    const area = selection ?? documentBounds(present);
    const mode = translation.current();
    const layer = mode?.document === present ? activeLayer(mode) : null;
    const pixels = renderScenePixels(
      {
        bitmap: image.bitmap,
        document: present,
        translation: layer ?? void 0,
      },
      area,
    );

    const result = await exportScreenshot({ action, area, label, pixels });

    return { document, result, translated: layer !== null };
  };

  const startTextSelection = useMemoizedFn(async (document: EditorDocument) => {
    if (!image || closingRef.current) return;

    const generation = textRequestGenerationRef.current + 1;
    textRequestGenerationRef.current = generation;
    const area = documentBounds(document);
    const mode: TextSelectionMode = {
      area,
      document,
      loading: true,
      regions: [],
    };
    textSelectionModeRef.current = mode;
    setTextSelectionMode(mode);
    setBusyAction("selectText");

    try {
      const pixels = renderScenePixels(
        { bitmap: image.bitmap, document },
        area,
      );
      const result = await exportScreenshot({
        action: "selectText",
        area,
        label,
        pixels,
      });
      if (
        generation !== textRequestGenerationRef.current ||
        closingRef.current ||
        presentRef.current !== document
      ) {
        return;
      }

      const next: TextSelectionMode = {
        ...mode,
        loading: false,
        regions: validTextRegions(result.textRegions, area),
      };
      textSelectionModeRef.current = next;
      setTextSelectionMode(next);
    } catch (error) {
      if (generation === textRequestGenerationRef.current) {
        leaveTextSelection();
      }
      log.error("select screenshot image text failed", error);
    } finally {
      if (generation === textRequestGenerationRef.current) {
        setBusyAction(null);
      }
    }
  });

  const handleSelectText = () => {
    if (textSelectionModeRef.current || busyAction === "selectText") {
      leaveTextSelection();
      return;
    }
    if (!image || !present || busyAction) return;

    translation.leave();
    setSelection(null);
    setSelectedId(null);
    setToolOptionsOpen(false);
    if (textEditRef.current) {
      commitTextEdit();
      pendingTextStartRef.current = true;
      return;
    }

    void startTextSelection(present);
  };

  useEffect(() => {
    if (!pendingTextStartRef.current || textEdit !== null || !present) return;

    pendingTextStartRef.current = false;
    void startTextSelection(present);
  }, [present, startTextSelection, textEdit]);

  useEffect(() => {
    if (textSelectionMode && textSelectionMode.document !== present) {
      leaveTextSelection();
    }
  }, [leaveTextSelection, present, textSelectionMode]);

  useEffect(() => {
    if (
      translationSession &&
      (translationSession.document !== present ||
        translationSession.selection !== selection)
    ) {
      translation.leave();
    }
  }, [translation.leave, present, selection, translationSession]);

  const updateSelectedImageText = useMemoizedFn((text: string) => {
    selectedImageTextRef.current = text;
    setSelectedImageTextValue(text);
  });

  const copySelectedImageText = async () => {
    const mode = textSelectionModeRef.current;
    if (!mode || mode.loading) return false;

    const text =
      selectedImageText(mode.regions, textOverlayRef.current) ||
      selectedImageTextRef.current;
    if (!text.trim()) {
      getMessageApi().info(t("editor.textSelect.hint"));
      return false;
    }

    try {
      await copyScreenshotText(label, text);
      getMessageApi().success(t("commands:messages.copied"));
      return true;
    } catch (error) {
      log.error("copy selected screenshot text failed", error);
      return false;
    }
  };

  const handleTranslate = async () => {
    if (translation.current() || busyAction === "translate") {
      translation.leave();
      return;
    }
    if (!image || !present || busyAction) return;

    leaveTextSelection();
    await translation.begin(present, selection);
  };

  const handleSaveTranslationKey = async (key: string) => {
    if (translationKeyBusy || translationKeySource === "environment") return;
    setTranslationKeyBusy(true);
    let saved = false;

    try {
      await setScreenshotTranslationKey(key);
      saved = true;
      const status = await getScreenshotTranslationStatus();
      applyTranslationKeyStatus(status);
      if (!status.configured) return;
      setTranslationKeyModalOpen(false);
      translation.resumeAfterKey();
    } catch (error) {
      log.error(
        saved
          ? "refresh screenshot translation key status after save failed"
          : "save screenshot translation key failed",
        error,
      );
    } finally {
      setTranslationKeyBusy(false);
    }
  };

  const handleRemoveTranslationKey = async () => {
    if (translationKeyBusy || translationKeySource === "environment") return;
    setTranslationKeyBusy(true);
    let removed = false;

    try {
      await removeScreenshotTranslationKey();
      removed = true;
      const status = await getScreenshotTranslationStatus();
      applyTranslationKeyStatus(status);
      setTranslationKeyModalOpen(false);
      translation.leave();
    } catch (error) {
      log.error(
        removed
          ? "refresh screenshot translation key status after removal failed"
          : "remove screenshot translation key failed",
        error,
      );
    } finally {
      setTranslationKeyBusy(false);
    }
  };

  const cancelTranslationKeyModal = () => {
    if (isAwaitingKey(translation.current())) translation.leave();
    setTranslationKeyModalOpen(false);
    setBusyAction((current) => {
      return current === "translate" ? null : current;
    });
  };

  const openTranslationKeyModal = async () => {
    try {
      const status = await getScreenshotTranslationStatus();
      if (closingRef.current) return;

      applyTranslationKeyStatus(status);
      setTranslationKeyModalOpen(true);
    } catch (error) {
      log.error("read screenshot translation key status failed", error);
    }
  };

  const retryTranslation = () => {
    if (busyAction) return;

    translation.retry();
  };

  const copyTranslationText = async () => {
    const session = translation.current();
    if (!session || isLoading(session)) return;

    const text = translatedText(session);
    if (!text) return;

    try {
      await copyScreenshotText(label, text);
      getMessageApi().success(t("editor.translation.copied"));
    } catch (error) {
      log.error("copy screenshot translation failed", error);
    }
  };

  const runExport = async (action: WindowExportAction): Promise<boolean> => {
    if (busyAction || !present) return false;

    setBusyAction(action);

    try {
      const exported = await renderAndExport(action);
      if (!exported) return false;
      const { document, result, translated } = exported;

      if (action === "ocr") {
        const text = result.text ?? "";
        if (text) {
          getMessageApi().success(
            t("editor.ocrCopied", { count: Array.from(text).length }),
          );
        } else {
          getMessageApi().warning(t("editor.ocrEmpty"));
        }
        return Boolean(text);
      }

      if ((action === "save" || action === "saveAs") && !result.savedPath) {
        return false;
      }

      exportStateRef.current = recordExport(exportStateRef.current, {
        action,
        current: presentRef.current,
        document,
        savedPath: result.savedPath,
        translated,
      });

      if (action === "copy") {
        getMessageApi().success(t("commands:messages.copied"));
      }

      if ((action === "save" || action === "saveAs") && result.savedPath) {
        getMessageApi().success(
          t("editor.saved", {
            name:
              action === "save"
                ? result.savedPath
                : fileNameOf(result.savedPath),
          }),
        );
      }
      return true;
    } catch (error) {
      log.error(`screenshot ${action} failed`, error);
      return false;
    } finally {
      setBusyAction(null);
    }
  };

  const wantedDragIdentity = (): DragIdentity => {
    return {
      document: present,
      selection,
      translation: translation.activeLayer(),
    };
  };

  const liveDragIdentity = (): DragIdentity => {
    return {
      document: dragDocumentRef.current,
      selection: dragSelectionRef.current,
      translation: translation.activeLayer(),
    };
  };

  const prepareDrag = (wanted = wantedDragIdentity()) => {
    if (!present) return null;

    return dragPreparationRef.current.prepare(
      wanted,
      liveDragIdentity,
      async () => {
        return (await renderAndExport("prepareDrag")) !== null;
      },
      (error) => {
        log.error("prepare screenshot drag failed", error);
      },
    );
  };

  const handleDragPointerEnter = () => {
    void prepareDrag();
  };

  const handleDragPointerDown = (event: ReactPointerEvent<HTMLElement>) => {
    if (event.button !== 0) return;

    event.preventDefault();
    dragGestureRef.current.down(event.pointerId, event.clientX, event.clientY);
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const handleDragPointerMove = async (
    event: ReactPointerEvent<HTMLElement>,
  ) => {
    const pointer = {
      buttons: event.buttons,
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
    };
    const handle = event.currentTarget;
    const gesture = wantedDragIdentity();
    const stillCurrent = () => {
      return sameDragIdentity(liveDragIdentity(), gesture);
    };
    try {
      await dragGestureRef.current.move(
        pointer,
        async () => {
          const prepared = await prepareDrag(gesture);
          return Boolean(prepared) && stillCurrent();
        },
        async () => {
          if (!stillCurrent()) return;
          if (handle.hasPointerCapture(pointer.pointerId)) {
            handle.releasePointerCapture(pointer.pointerId);
          }
          await startScreenshotDrag(label);
        },
      );
    } catch (error) {
      log.error("start screenshot drag failed", error);
    }
  };

  const cancelDragGesture = () => {
    dragGestureRef.current.cancel();
  };

  const handleBrowserBackPointerDown = (event: PointerEvent) => {
    if (event.button === 3) cancelDragGesture();
  };

  const handleBrowserBackMouseDown = (event: MouseEvent) => {
    if (event.button === 3) cancelDragGesture();
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
    leaveTextSelection();
    translation.leave();
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
    leaveTextSelection();
    translation.leave();
    setSelection(null);
    setSelectedId(null);
    setDraft(null);
    setHistory((current) => {
      return current ? undoHistory(current) : current;
    });
  };

  const redo = () => {
    leaveTextSelection();
    translation.leave();
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
    leaveTextSelection();
    translation.leave();
    setTranslationKeyModalOpen(false);

    try {
      const cancelSettled = translation.cancelSettled();
      if (cancelSettled) {
        await Promise.race([
          cancelSettled,
          new Promise<void>((resolve) => {
            window.setTimeout(resolve, 1000);
          }),
        ]);
      }
      await closeScreenshotWindow(label);
    } catch (error) {
      closingRef.current = false;
      log.error("close screenshot editor failed", error);
    }
  };

  const requestClose = () => {
    if (closingRef.current) return;

    leaveTextSelection();
    translation.leave();
    setTranslationKeyModalOpen(false);
    commitTextEdit();

    if (!hasUnexportedChanges(exportStateRef.current, present)) {
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

  const runCommand = (command: EditorCommand) => {
    switch (command.type) {
      case "actualSize":
        showActualSize();
        return;
      case "cancelDrag":
        cancelDragGesture();
        return;
      case "clearSelection":
        setSelection(null);
        return;
      case "close":
        requestClose();
        return;
      case "copy":
        void runExport("copy");
        return;
      case "copyAndClose":
        void handleCopyAndClose();
        return;
      case "copyColor":
        void copyHoverColor();
        return;
      case "copyImageText":
        void copySelectedImageText();
        return;
      case "crop":
        cropToSelection();
        return;
      case "deleteShape":
        deleteSelectedShape();
        return;
      case "deselectShape":
        setSelectedId(null);
        return;
      case "fit":
        fit();
        return;
      case "holdPan":
        setPanHeld(true);
        return;
      case "holdZoom":
        setZoomHeld(true);
        return;
      case "leaveTextSelection":
        leaveTextSelection();
        return;
      case "leaveTranslation":
        translation.leave();
        return;
      case "nudgeSelection":
        if (selection && present) {
          setSelection(
            nudgeRect(
              selection,
              command.key,
              command.step,
              command.extend,
              present.crop,
            ),
          );
        }
        return;
      case "nudgeShape":
        nudgeSelectedShape(command.key, command.step);
        return;
      case "ocr":
        void runExport("ocr");
        return;
      case "paste":
        void pasteImage();
        return;
      case "redo":
        redo();
        return;
      case "save":
        void runExport("save");
        return;
      case "selectTool":
        changeTool(command.tool);
        return;
      case "undo":
        undo();
        return;
      case "zoom":
        zoomBy(command.direction === "in" ? ZOOM_STEP : 1 / ZOOM_STEP);
        return;
    }
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    const mode: KeyMode = translation.current()
      ? "translation"
      : textSelectionModeRef.current || pendingTextStartRef.current
        ? "textSelection"
        : "edit";
    const action = resolveKey(event, {
      dragActive: dragGestureRef.current.active(),
      hasDocument: present !== null,
      hasSelectedId: selectedId !== null,
      hasSelectedShape: present
        ? findShape(present, selectedId) !== null
        : false,
      hasSelection: selection !== null,
      keyModalOpen: translationKeyModalOpen,
      mode,
      target: keyTargetOf(event.target),
      textEditing: textEditRef.current !== null,
      tool,
    });
    if (!action) return;

    if (action.preventDefault) event.preventDefault();
    runCommand(action.command);
  };

  const handleKeyUp = (event: KeyboardEvent) => {
    if (event.code === "Space") setPanHeld(false);
    if (event.code === "KeyZ") setZoomHeld(false);
  };

  const handleWindowBlur = () => {
    cancelDragGesture();
    setPanHeld(false);
    setZoomHeld(false);
  };

  const preventUnsupportedDrop = (event: DragEvent) => {
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "none";
  };

  useEventListener("keydown", handleKeyDown);
  useEventListener("keyup", handleKeyUp);
  useEventListener("blur", handleWindowBlur);
  useEventListener("pointerup", cancelDragGesture);
  useEventListener("pointercancel", cancelDragGesture);
  useEventListener("popstate", cancelDragGesture);
  useEventListener("pointerdown", handleBrowserBackPointerDown);
  useEventListener("mousedown", handleBrowserBackMouseDown);
  useEventListener("dragover", preventUnsupportedDrop);
  useEventListener("drop", preventUnsupportedDrop);

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
    if (textSelectionModeRef.current) {
      void copySelectedImageText();
      return;
    }

    void runExport("copy");
  };

  const handleCopyAndClose = async () => {
    if (textSelectionModeRef.current) {
      if (await copySelectedImageText()) await closeWindow();
      return;
    }

    const copied = await runExport("copy");
    if (copied && presentRef.current === present) {
      await closeWindow();
    }
  };

  const handleDiscardAndClose = () => {
    void closeWindow();
  };

  const handleSave = () => {
    void runExport("save");
  };

  const handleSaveAs = () => {
    void runExport("saveAs");
  };

  const handlePin = () => {
    void runExport("pin");
  };

  const handlePasteImage = () => {
    void pasteImage();
  };

  const handleRecognizeText = () => {
    leaveTextSelection();
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
        onDragPointerMove={handleDragPointerMove}
        onMaximize={toggleMaximizeWindow}
        onMinimize={minimizeWindow}
        onMoveWindow={moveWindow}
        onPasteImage={handlePasteImage}
        onPin={handlePin}
        onRecognizeText={handleRecognizeText}
        onSave={handleSave}
        onSaveAs={handleSaveAs}
        onSelectText={handleSelectText}
        onToolChange={changeTool}
        onTranslate={handleTranslate}
        onZoomToggle={toggleZoom}
        textSelectionActive={textSelectionMode !== null}
        tool={tool}
        translationActive={translationSession !== null}
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
          translation: activeTranslation ?? void 0,
        }}
        selectedShape={selectedShape}
        selection={selection}
        viewport={viewport}
        zoomHeld={zoomHeld}
      >
        {textSelectionMode && viewport && viewSize && (
          <ImageTextSelection
            area={textSelectionMode.area}
            loading={textSelectionMode.loading}
            onCopy={copySelectedImageText}
            onLeave={leaveTextSelection}
            onSelectedText={updateSelectedImageText}
            ratio={viewSize.ratio}
            regions={textSelectionMode.regions}
            rootRef={textOverlayRef}
            selectedText={selectedImageTextValue}
            viewport={viewport}
          />
        )}
        {translationSession && (
          <TranslationControls
            blocks={translationSession.blocks}
            failed={translationSession.phase === "failed"}
            loading={isLoading(translationSession)}
            onCopyText={copyTranslationText}
            onLeave={translation.leave}
            onManageKey={openTranslationKeyModal}
            onRetry={retryTranslation}
            onToggleOriginal={translation.toggleShowOriginal}
            overflowCount={translationSession.layer?.overflowCount ?? 0}
            showOriginal={translationSession.showOriginal}
          />
        )}
        {!textSelectionMode && !translationSession && subject && (
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
      <TranslationKeyModal
        busy={translationKeyBusy}
        configured={translationKeyConfigured}
        model={translationKeyModel}
        onCancel={cancelTranslationKeyModal}
        onRemove={handleRemoveTranslationKey}
        onSave={handleSaveTranslationKey}
        open={translationKeyModalOpen}
        source={translationKeySource}
      />
    </div>
  );
};

function keyTargetOf(target: EventTarget | null): KeyTarget {
  if (!(target instanceof HTMLElement)) {
    return { inDialog: false, inField: false };
  }

  return {
    inDialog: target.closest('[role="dialog"]') !== null,
    inField:
      target.closest("input, textarea, [contenteditable='true']") !== null ||
      target.closest("[data-editor-options]") !== null,
  };
}

function fileNameOf(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

export default ScreenshotEditor;
