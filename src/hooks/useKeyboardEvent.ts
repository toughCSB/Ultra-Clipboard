import { useEventListener, useLatest } from "ahooks";
import { TAURI_EVENT } from "@/constants/events";
import { isWinClipboardWindow } from "@/utils/is";
import { useTauriListen } from "./useTauriListen";

type KeyboardEventType = "keydown" | "keyup";

const EDITABLE_GLOBAL_KEYBOARD_ATTRIBUTE = "data-allow-global-keyboard";
const EDITABLE_GLOBAL_KEYBOARD_SELECTOR = `[${EDITABLE_GLOBAL_KEYBOARD_ATTRIBUTE}="true"]`;
const EDITABLE_GLOBAL_HANDOFF_KEYS = new Set([
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "ArrowUp",
  "Control",
  "Enter",
  "Escape",
  "Tab",
]);

interface NavEventPayload {
  code?: string;
  type: KeyboardEventType;
  key: string;
  ctrlKey?: boolean;
  shiftKey?: boolean;
}

export const useKeyboardEvent = (
  type: KeyboardEventType,
  handler: (event: KeyboardEvent) => void,
) => {
  const isWindowsClipboardWindow = isWinClipboardWindow();
  const handlerRef = useLatest(handler);

  const handleBrowserEvent = (event: KeyboardEvent) => {
    if (isWindowsClipboardWindow) {
      const editableTarget = findEditableElement(event.target);
      if (editableTarget) {
        if (!shouldHandoffEditableKeyboard(editableTarget, event)) return;

        editableTarget.blur();
      }
    }

    handlerRef.current(event);
  };

  useEventListener(type, handleBrowserEvent);

  useTauriListen<NavEventPayload>(TAURI_EVENT.KEYBOARD_NAV, (event) => {
    if (!isWindowsClipboardWindow) return;
    if (shouldUseNativeEditableKeyboard(document.activeElement)) return;

    const { type: payloadType, ...rest } = event.payload;

    if (payloadType !== type || !rest.key) return;

    handlerRef.current(
      new KeyboardEvent(payloadType, { cancelable: true, ...rest }),
    );
  });
};

function findEditableElement(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof Element)) return null;

  let element: Element | null = target;
  while (element) {
    if (element instanceof HTMLElement) {
      if (element.isContentEditable) return element;

      const tagName = element.tagName.toLowerCase();
      if (tagName === "input" || tagName === "textarea") return element;
    }

    element = element.parentElement;
  }

  return null;
}

function shouldUseNativeEditableKeyboard(target: EventTarget | null) {
  return findEditableElement(target) !== null;
}

function shouldHandoffEditableKeyboard(
  target: HTMLElement,
  event: KeyboardEvent,
) {
  if (event.type !== "keydown") return false;
  if (!target.closest(EDITABLE_GLOBAL_KEYBOARD_SELECTOR)) return false;

  return EDITABLE_GLOBAL_HANDOFF_KEYS.has(event.key);
}
