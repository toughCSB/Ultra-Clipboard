import { proxy } from "valtio";

interface ClipboardStatsState {
  /** Total items under the current List filters, or null before the first load. */
  total: number | null;
}

/** Shared List/Footer state for the total returned by the Rust query. */
export const clipboardStatsState = proxy<ClipboardStatsState>({
  total: null,
});
