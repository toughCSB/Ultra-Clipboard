import { proxy } from "valtio";
import type { ClipboardCategory, ClipboardRange } from "@/types/clipboard";

interface ClipboardViewState {
  category: ClipboardCategory | null;
  keyword: string;
  groupId: string | null;
  range: ClipboardRange;
}

/** Non-persistent clipboard-window UI state shared by Header, Group, and List. */
export const clipboardViewState = proxy<ClipboardViewState>({
  category: null,
  groupId: null,
  keyword: "",
  range: "all",
});
