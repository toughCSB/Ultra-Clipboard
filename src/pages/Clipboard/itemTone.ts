/* @unocss-include */
import type { ClipboardKind } from "@/types/clipboard";

interface ItemTone {
  card: string;
  cardSelected: string;
  pill: string;
  stripe: string;
}

const TONES: Record<"files" | "image" | "text", ItemTone> = {
  files: {
    card: "border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-950/50",
    cardSelected:
      "border-amber-500 bg-amber-100 dark:border-amber-400 dark:bg-amber-900/70",
    pill: "bg-amber-500 text-white",
    stripe: "bg-amber-500",
  },
  image: {
    card: "border-cyan-300 bg-cyan-50 dark:border-cyan-700 dark:bg-cyan-950/50",
    cardSelected:
      "border-cyan-500 bg-cyan-100 dark:border-cyan-400 dark:bg-cyan-900/70",
    pill: "bg-cyan-500 text-white",
    stripe: "bg-cyan-500",
  },
  text: {
    card: "border-violet-300 bg-violet-50 dark:border-violet-700 dark:bg-violet-950/50",
    cardSelected:
      "border-violet-500 bg-violet-100 dark:border-violet-400 dark:bg-violet-900/70",
    pill: "bg-violet-500 text-white",
    stripe: "bg-violet-500",
  },
};

export function resolveItemTone(kind: ClipboardKind): ItemTone {
  if (kind === "image") return TONES.image;
  if (kind === "files") return TONES.files;

  return TONES.text;
}

export function resolveFilterSelectedClass(value: string): string {
  if (value === "favorite") return "bg-amber-500 text-white";
  if (value === "image") return "bg-cyan-500 text-white";
  if (value === "files") return "bg-orange-500 text-white";
  if (value === "text") return "bg-violet-500 text-white";

  return "bg-teal-500 text-white";
}
