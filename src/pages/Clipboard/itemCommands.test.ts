import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { ClipboardAction } from "@/types/clipboard";
import type { ItemAction } from "@/types/settings";
import {
  closesPreview,
  commandFromClickSetting,
  commandFromMenuAction,
  commandFromQuickAction,
  type ItemCommand,
} from "./itemCommands";

describe("itemCommands", () => {
  it("maps every context-menu action", () => {
    const expected: Record<ClipboardAction, ItemCommand | null> = {
      copy: { plain: false, type: "copy" },
      delete: { type: "delete" },
      editNote: { type: "editNote" },
      moveToGroup: { groupId: "g1", type: "moveToGroup" },
      openLink: { mailto: false, type: "openLink" },
      paste: { plain: false, type: "paste" },
      pasteAsPath: { plain: true, type: "paste" },
      pasteAsPlainText: { plain: true, type: "paste" },
      revealInExplorer: { type: "reveal" },
      revealInFinder: { type: "reveal" },
      saveImage: { type: "saveImage" },
      sendEmail: { mailto: true, type: "openLink" },
      toggleFavorite: { type: "toggleFavorite" },
      togglePinned: { type: "togglePinned" },
    };

    for (const [action, command] of Object.entries(expected)) {
      assert.deepEqual(
        commandFromMenuAction(action as ClipboardAction, "g1"),
        command,
        action,
      );
    }
  });

  it("ignores a move to group without a target group", () => {
    assert.equal(commandFromMenuAction("moveToGroup"), null);
  });

  it("maps every quick action", () => {
    const expected: Record<ItemAction, ItemCommand> = {
      copy: { plain: false, type: "copy" },
      copyPlain: { plain: true, type: "copy" },
      delete: { type: "delete" },
      note: { type: "editNote" },
      openLink: { mailto: false, type: "openLink" },
      paste: { plain: false, type: "paste" },
      pastePath: { plain: true, type: "paste" },
      pastePlain: { plain: true, type: "paste" },
      pinItem: { type: "togglePinned" },
      reveal: { type: "reveal" },
      sendEmail: { mailto: true, type: "openLink" },
      star: { type: "toggleFavorite" },
    };

    for (const [action, command] of Object.entries(expected)) {
      assert.deepEqual(
        commandFromQuickAction(action as ItemAction),
        command,
        action,
      );
    }
  });

  it("maps each click setting only for its click count", () => {
    const paste: ItemCommand = { plain: false, type: "paste" };
    const pastePlain: ItemCommand = { plain: true, type: "paste" };
    const copy: ItemCommand = { plain: false, type: "copy" };
    const copyPlain: ItemCommand = { plain: true, type: "copy" };

    assert.equal(commandFromClickSetting("disabled", "single"), null);
    assert.equal(commandFromClickSetting("disabled", "double"), null);
    assert.deepEqual(
      commandFromClickSetting("singleClickPaste", "single"),
      paste,
    );
    assert.equal(commandFromClickSetting("singleClickPaste", "double"), null);
    assert.deepEqual(
      commandFromClickSetting("singleClickPastePlain", "single"),
      pastePlain,
    );
    assert.deepEqual(
      commandFromClickSetting("singleClickCopy", "single"),
      copy,
    );
    assert.deepEqual(
      commandFromClickSetting("singleClickCopyPlain", "single"),
      copyPlain,
    );
    assert.deepEqual(
      commandFromClickSetting("doubleClickPaste", "double"),
      paste,
    );
    assert.equal(commandFromClickSetting("doubleClickPaste", "single"), null);
    assert.deepEqual(
      commandFromClickSetting("doubleClickCopy", "double"),
      copy,
    );
    assert.equal(commandFromClickSetting("doubleClickCopy", "single"), null);
  });

  it("always closes the preview for a paste", () => {
    const paste: ItemCommand = { plain: false, type: "paste" };

    assert.equal(closesPreview(paste, null, "a"), true);
    assert.equal(closesPreview(paste, "b", "a"), true);
  });

  it("keeps the preview open for favorite and pin toggles", () => {
    assert.equal(closesPreview({ type: "toggleFavorite" }, "a", "a"), false);
    assert.equal(closesPreview({ type: "togglePinned" }, "a", "a"), false);
  });

  it("closes the preview for other commands only on the previewed item", () => {
    const copy: ItemCommand = { plain: false, type: "copy" };

    assert.equal(closesPreview(copy, "a", "a"), true);
    assert.equal(closesPreview(copy, "b", "a"), false);
    assert.equal(closesPreview({ type: "delete" }, null, "a"), false);
  });
});
