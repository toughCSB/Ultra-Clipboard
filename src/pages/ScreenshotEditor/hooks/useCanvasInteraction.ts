import { useRef } from "react";
import {
  hitRectHandle,
  moveRectInside,
  type PixelPoint,
  type PixelRect,
  type RectHandle,
  rectContainsPoint,
  rectFromEdges,
  resizeRectToEdge,
  snapEdgeInside,
} from "@/utils/pixelRect";
import {
  CLICK_DISTANCE,
  createShape,
  type DragInfo,
  dragCreatedShape,
  finalizeCreatedShape,
  grabsShape,
  isSelectTool,
  measureRulers,
  refreshShape,
  type SourceImage,
} from "../model/creation";
import {
  addShape,
  addShapes,
  documentBounds,
  type EditorDocument,
  findShape,
  replaceShape,
} from "../model/document";
import { measureSpans, type PixelSpans } from "../model/pixels";
import {
  hitShapeHandle,
  moveShape,
  resizeShape,
  type Shape,
  type ShapeHandle,
  topShapeAt,
} from "../model/shapes";
import type { EditorTool, ToolStyle } from "../model/tools";

export interface PointerInfo extends DragInfo {
  altKey: boolean;
}

export interface TextEditRequest {
  point: PixelPoint;
  shape: Extract<Shape, { kind: "text" }> | null;
}

export interface CanvasInteraction {
  cursor: (point: PixelPoint, info: PointerInfo) => string;
  doubleClick: (point: PixelPoint, info: PointerInfo) => void;
  hover: (point: PixelPoint | null) => void;
  pointerDown: (point: PixelPoint, info: PointerInfo) => void;
  pointerMove: (point: PixelPoint, info: PointerInfo) => void;
  pointerUp: (point: PixelPoint, info: PointerInfo) => void;
}

interface InteractionOptions {
  document: EditorDocument;
  image: SourceImage;
  /** Device pixels per logical pixel of the capture, used to size new annotations. */
  scale: number;
  selectedId: string | null;
  selection: PixelRect | null;
  style: ToolStyle;
  tool: EditorTool;
  commit: (document: EditorDocument) => void;
  onEditText: (request: TextEditRequest) => void;
  onMeasure: (spans: PixelSpans | null) => void;
  onSelectShape: (id: string | null) => void;
  onSelectionChange: (selection: PixelRect | null) => void;
  setDraft: (document: EditorDocument | null) => void;
}

type Gesture =
  | { kind: "area"; origin: PixelPoint }
  | { kind: "moveArea"; origin: PixelPoint; rect: PixelRect }
  | { handle: RectHandle; kind: "resizeArea"; rect: PixelRect }
  | { kind: "moveShape"; moved: boolean; origin: PixelPoint; shape: Shape }
  | { handle: ShapeHandle; kind: "resizeShape"; shape: Shape }
  | { kind: "create"; origin: PixelPoint; shape: Shape };

const HANDLE_CURSORS: Record<RectHandle, string> = {
  e: "ew-resize",
  n: "ns-resize",
  ne: "nesw-resize",
  nw: "nwse-resize",
  s: "ns-resize",
  se: "nwse-resize",
  sw: "nesw-resize",
  w: "ew-resize",
};

/** Colors that differ by at most this much in every channel count as one surface. */
const MEASURE_TOLERANCE = 12;

/**
 * Turns pointer input in image coordinates into selections and annotations.
 * Drags show a draft document; releasing the pointer commits one undo step.
 */
export const useCanvasInteraction = (
  options: InteractionOptions,
): CanvasInteraction => {
  const gestureRef = useRef<Gesture | null>(null);
  const draftRef = useRef<EditorDocument | null>(null);
  const optionsRef = useRef(options);

  optionsRef.current = options;

  const setDraft = (document: EditorDocument | null) => {
    draftRef.current = document;
    optionsRef.current.setDraft(document);
  };

  const finishDraft = () => {
    const draft = draftRef.current;
    setDraft(null);
    if (draft) optionsRef.current.commit(draft);
  };

  const bounds = () => documentBounds(optionsRef.current.document);

  const measureAt = (point: PixelPoint) => {
    const { document, image } = optionsRef.current;

    return measureSpans(
      image.pixels,
      image.width,
      image.height,
      point,
      document.crop,
      MEASURE_TOLERANCE,
    );
  };

  const commitMeasure = (point: PixelPoint) => {
    const { document, scale, style } = optionsRef.current;
    const spans = measureAt(point);
    if (!spans) return;

    optionsRef.current.commit(
      addShapes(document, measureRulers(spans, style, scale)),
    );
  };

  /** Starts moving or resizing the selected shape, or selects the shape under the press. */
  const beginShapeGesture = (point: PixelPoint, info: PointerInfo) => {
    const { document, selectedId, tool } = optionsRef.current;
    const selected = findShape(document, selectedId);

    if (selected) {
      const handle = hitShapeHandle(selected, point, info.tolerance);
      if (handle) {
        gestureRef.current = { handle, kind: "resizeShape", shape: selected };
        return true;
      }
    }

    const hit = topShapeAt(document.shapes, point, info.tolerance);
    const target = hit && grabsShape(hit, tool, selectedId) ? hit : null;
    if (!target) return false;

    optionsRef.current.onSelectShape(target.id);
    gestureRef.current = {
      kind: "moveShape",
      moved: false,
      origin: point,
      shape: target,
    };

    return true;
  };

  const pointerDown = (point: PixelPoint, info: PointerInfo) => {
    const { document, scale, selection, style, tool } = optionsRef.current;

    if (isSelectTool(tool)) {
      if (beginShapeGesture(point, info)) return;

      optionsRef.current.onSelectShape(null);

      if (selection) {
        const handle = hitRectHandle(selection, point, info.tolerance);
        if (handle) {
          gestureRef.current = { handle, kind: "resizeArea", rect: selection };
          return;
        }
        if (rectContainsPoint(selection, point)) {
          gestureRef.current = {
            kind: "moveArea",
            origin: point,
            rect: selection,
          };
          return;
        }
      }

      gestureRef.current = {
        kind: "area",
        origin: snapEdgeInside(point, bounds()),
      };
      optionsRef.current.onSelectionChange(null);
      return;
    }

    optionsRef.current.onSelectionChange(null);

    if (tool === "text") {
      const hit = topShapeAt(document.shapes, point, info.tolerance);
      if (hit?.kind === "text") {
        optionsRef.current.onSelectShape(hit.id);
        gestureRef.current = {
          kind: "moveShape",
          moved: false,
          origin: point,
          shape: hit,
        };
        return;
      }

      optionsRef.current.onSelectShape(null);
      optionsRef.current.onEditText({ point, shape: null });
      return;
    }

    // The pen never starts a shape gesture, not even on the selected shape's handles.
    if (tool !== "pen" && beginShapeGesture(point, info)) return;

    const shape = createShape(tool, point, style, scale, document.shapes);
    if (!shape) return;

    optionsRef.current.onSelectShape(null);
    optionsRef.current.onMeasure(null);
    gestureRef.current = { kind: "create", origin: point, shape };
    if (shape.kind === "pen") setDraft(addShape(document, shape));
  };

  const pointerMove = (point: PixelPoint, info: PointerInfo) => {
    const gesture = gestureRef.current;
    const { document, image } = optionsRef.current;

    if (!gesture) return;

    switch (gesture.kind) {
      case "area": {
        const rect = rectFromEdges(
          gesture.origin,
          snapEdgeInside(point, bounds()),
        );
        optionsRef.current.onSelectionChange(
          rect.width > 0 && rect.height > 0 ? rect : null,
        );
        return;
      }
      case "moveArea": {
        const dx = Math.round(point.x - gesture.origin.x);
        const dy = Math.round(point.y - gesture.origin.y);
        optionsRef.current.onSelectionChange(
          moveRectInside(gesture.rect, dx, dy, bounds()),
        );
        return;
      }
      case "resizeArea": {
        const rect = resizeRectToEdge(
          gesture.rect,
          gesture.handle,
          point,
          bounds(),
        );
        optionsRef.current.onSelectionChange(
          rect.width > 0 && rect.height > 0 ? rect : null,
        );
        return;
      }
      case "moveShape": {
        let dx = point.x - gesture.origin.x;
        let dy = point.y - gesture.origin.y;
        if (
          !gesture.moved &&
          Math.hypot(dx, dy) < info.tolerance * CLICK_DISTANCE
        ) {
          return;
        }
        if (info.shiftKey) {
          if (Math.abs(dx) > Math.abs(dy)) dy = 0;
          else dx = 0;
        }

        gestureRef.current = { ...gesture, moved: true };
        setDraft(
          replaceShape(
            document,
            refreshShape(moveShape(gesture.shape, dx, dy), image),
          ),
        );
        return;
      }
      case "resizeShape":
        setDraft(
          replaceShape(
            document,
            refreshShape(
              resizeShape(gesture.shape, gesture.handle, point, info.shiftKey),
              image,
            ),
          ),
        );
        return;
      case "create": {
        const shape = dragCreatedShape(
          gesture.shape,
          gesture.origin,
          point,
          info,
        );
        gestureRef.current = { ...gesture, shape };
        setDraft(addShape(document, shape));
        return;
      }
    }
  };

  const pointerUp = (point: PixelPoint, info: PointerInfo) => {
    const gesture = gestureRef.current;
    gestureRef.current = null;
    if (!gesture) return;

    const { document, image, scale } = optionsRef.current;

    switch (gesture.kind) {
      case "moveShape":
      case "resizeShape":
        finishDraft();
        return;
      case "create": {
        const shape = dragCreatedShape(
          gesture.shape,
          gesture.origin,
          point,
          info,
        );
        const moved =
          Math.hypot(point.x - gesture.origin.x, point.y - gesture.origin.y) >=
          info.tolerance * CLICK_DISTANCE;

        setDraft(null);

        if (shape.kind === "ruler" && !moved) {
          commitMeasure(point);
          return;
        }

        const finished = finalizeCreatedShape(
          shape,
          info,
          image,
          bounds(),
          scale,
        );
        if (!finished) return;

        optionsRef.current.commit(addShape(document, finished));
        if (finished.kind !== "pen") {
          optionsRef.current.onSelectShape(finished.id);
        }
        return;
      }
      default:
        return;
    }
  };

  const doubleClick = (point: PixelPoint, info: PointerInfo) => {
    const { document } = optionsRef.current;
    const hit = topShapeAt(document.shapes, point, info.tolerance);
    if (hit?.kind !== "text") return;

    gestureRef.current = null;
    setDraft(null);
    optionsRef.current.onSelectShape(hit.id);
    optionsRef.current.onEditText({ point, shape: hit });
  };

  const hover = (point: PixelPoint | null) => {
    const { tool } = optionsRef.current;
    if (tool !== "ruler") return;

    optionsRef.current.onMeasure(
      point && !gestureRef.current ? measureAt(point) : null,
    );
  };

  const cursor = (point: PixelPoint, info: PointerInfo) => {
    const { document, selectedId, selection, tool } = optionsRef.current;
    const selected = findShape(document, selectedId);

    if (selected) {
      const handle = hitShapeHandle(selected, point, info.tolerance);
      if (handle === "start" || handle === "end") return "grab";
      if (handle) return HANDLE_CURSORS[handle];
    }

    const hit = topShapeAt(document.shapes, point, info.tolerance);

    if (hit && grabsShape(hit, tool, selectedId)) return "move";

    if (tool === "text") return "text";
    if (!isSelectTool(tool)) return "crosshair";
    if (!selection) return "crosshair";

    const handle = hitRectHandle(selection, point, info.tolerance);
    if (handle) return HANDLE_CURSORS[handle];

    return rectContainsPoint(selection, point) ? "move" : "crosshair";
  };

  return { cursor, doubleClick, hover, pointerDown, pointerMove, pointerUp };
};
