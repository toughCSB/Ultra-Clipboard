interface PointerPosition {
  buttons: number;
  pointerId: number;
  x: number;
  y: number;
}

interface DragIntent {
  pointerId: number;
  startX: number;
  startY: number;
  starting: boolean;
}

const DRAG_THRESHOLD_SQUARED = 36;

/** Keeps a released or cancelled pointer from starting a late native drag. */
export function createDragGesture() {
  let intent: DragIntent | null = null;

  return {
    active() {
      return intent !== null;
    },
    cancel() {
      const wasActive = intent !== null;
      intent = null;
      return wasActive;
    },
    down(pointerId: number, x: number, y: number) {
      intent = { pointerId, starting: false, startX: x, startY: y };
    },
    async move(
      pointer: PointerPosition,
      prepare: () => Promise<boolean> | null,
      start: () => Promise<void>,
    ) {
      const pending = intent;
      if (
        !pending ||
        pending.pointerId !== pointer.pointerId ||
        pending.starting
      ) {
        return false;
      }
      if ((pointer.buttons & 1) === 0) {
        intent = null;
        return false;
      }
      const dx = pointer.x - pending.startX;
      const dy = pointer.y - pending.startY;
      if (dx * dx + dy * dy < DRAG_THRESHOLD_SQUARED) return false;

      pending.starting = true;
      try {
        const ready = await prepare();
        if (intent !== pending || !ready) return false;
        intent = null;
        await start();
        return true;
      } finally {
        if (intent === pending) intent = null;
      }
    },
  };
}
