import type { ClipboardAction } from "@/types/clipboard";
import type {
  AutoPaste,
  ItemAction,
  MiddleClickAction,
} from "@/types/settings";

/**
 * One clipboard item action, whichever menu, key, button, or click asked for
 * it.
 */
export type ItemCommand =
  | { type: "paste"; plain: boolean }
  | { type: "copy"; plain: boolean }
  | { type: "saveImage" }
  | { type: "openLink"; mailto: boolean }
  | { type: "reveal" }
  | { type: "toggleFavorite" }
  | { type: "togglePinned" }
  | { type: "moveToGroup"; groupId: string }
  | { type: "editNote" }
  | { type: "delete" };

export function commandFromMenuAction(
  action: ClipboardAction,
  groupId?: string,
): ItemCommand | null {
  switch (action) {
    case "paste":
      return { plain: false, type: "paste" };
    case "pasteAsPlainText":
    case "pasteAsPath":
      return { plain: true, type: "paste" };
    case "copy":
      return { plain: false, type: "copy" };
    case "saveImage":
      return { type: "saveImage" };
    case "openLink":
      return { mailto: false, type: "openLink" };
    case "sendEmail":
      return { mailto: true, type: "openLink" };
    case "revealInFinder":
    case "revealInExplorer":
      return { type: "reveal" };
    case "toggleFavorite":
      return { type: "toggleFavorite" };
    case "togglePinned":
      return { type: "togglePinned" };
    case "moveToGroup":
      return groupId ? { groupId, type: "moveToGroup" } : null;
    case "editNote":
      return { type: "editNote" };
    case "delete":
      return { type: "delete" };
  }
}

export function commandFromQuickAction(action: ItemAction): ItemCommand {
  switch (action) {
    case "paste":
      return { plain: false, type: "paste" };
    case "pastePlain":
    case "pastePath":
      return { plain: true, type: "paste" };
    case "copy":
      return { plain: false, type: "copy" };
    case "copyPlain":
      return { plain: true, type: "copy" };
    case "openLink":
      return { mailto: false, type: "openLink" };
    case "sendEmail":
      return { mailto: true, type: "openLink" };
    case "reveal":
      return { type: "reveal" };
    case "note":
      return { type: "editNote" };
    case "star":
      return { type: "toggleFavorite" };
    case "pinItem":
      return { type: "togglePinned" };
    case "delete":
      return { type: "delete" };
  }
}

/**
 * The auto-paste or middle-click setting for one click, `null` when the setting
 * is disabled or belongs to the other click count.
 */
export function commandFromClickSetting(
  setting: AutoPaste | MiddleClickAction,
  click: "single" | "double",
): ItemCommand | null {
  switch (setting) {
    case "singleClickPaste":
      return click === "single" ? { plain: false, type: "paste" } : null;
    case "singleClickPastePlain":
      return click === "single" ? { plain: true, type: "paste" } : null;
    case "singleClickCopy":
      return click === "single" ? { plain: false, type: "copy" } : null;
    case "singleClickCopyPlain":
      return click === "single" ? { plain: true, type: "copy" } : null;
    case "doubleClickPaste":
      return click === "double" ? { plain: false, type: "paste" } : null;
    case "doubleClickCopy":
      return click === "double" ? { plain: false, type: "copy" } : null;
    case "disabled":
      return null;
  }
}

/**
 * A paste moves focus to the target app, so the preview always closes. Toggles
 * keep it so the preview can show the new state. Anything else closes it only
 * when it shows the item being acted on.
 */
export function closesPreview(
  command: ItemCommand,
  previewItemId: string | null,
  itemId: string,
) {
  switch (command.type) {
    case "paste":
      return true;
    case "toggleFavorite":
    case "togglePinned":
      return false;
    default:
      return previewItemId === itemId;
  }
}
