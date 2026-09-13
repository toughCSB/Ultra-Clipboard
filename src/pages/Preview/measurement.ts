import type { RefObject } from "react";
import { useEffect, useLayoutEffect, useState } from "react";
import { PREVIEW_PANEL_FALLBACK_SIZE } from "./constants";

export interface PreviewMeasuredSize {
  height: number;
  width: number;
}
/** Observe the hidden measurement layer and sync its natural size to the panel. */
export function useMeasuredPanelSize(
  ref: RefObject<HTMLDivElement | null>,
): PreviewMeasuredSize {
  const [size, setSize] = useState<PreviewMeasuredSize>(
    PREVIEW_PANEL_FALLBACK_SIZE,
  );

  useEffect(() => {
    const node = ref.current;
    if (!node) return;

    syncMeasuredPanelSize(node, setSize);

    const observer = new ResizeObserver(() => {
      syncMeasuredPanelSize(node, setSize);
    });
    observer.observe(node);

    return () => {
      observer.disconnect();
    };
  }, [ref]);

  useLayoutEffect(() => {
    const node = ref.current;
    if (!node) return;

    syncMeasuredPanelSize(node, setSize);
  });

  return size;
}
/** Whether the hidden measurement layer has reported the current natural size. */
export function hasMeasuredPanelSize(size: PreviewMeasuredSize) {
  return (
    size.height !== PREVIEW_PANEL_FALLBACK_SIZE.height ||
    size.width !== PREVIEW_PANEL_FALLBACK_SIZE.width
  );
}
/** Read natural dimensions without updating state when the size is unchanged. */
function syncMeasuredPanelSize(
  node: HTMLDivElement,
  setSize: React.Dispatch<React.SetStateAction<PreviewMeasuredSize>>,
) {
  const rect = node.getBoundingClientRect();
  const nextSize = {
    height: Math.ceil(rect.height),
    width: Math.ceil(rect.width),
  };

  setSize((currentSize) => {
    if (
      currentSize.height === nextSize.height &&
      currentSize.width === nextSize.width
    ) {
      return currentSize;
    }

    return nextSize;
  });
}
