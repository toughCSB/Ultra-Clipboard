import { motion, useReducedMotion } from "motion/react";
import type { FC, ReactNode } from "react";
import { useLayoutEffect, useRef, useState } from "react";
import { cn } from "@/utils/cn";
import NoteAnnotation from "./NoteAnnotation";

interface NoteContentSwitcherProps {
  note: string;

  children: ReactNode;

  showOriginal: boolean;
}

/** Show a note by default and the original content while hovering the content area. */
const NoteContentSwitcher: FC<NoteContentSwitcherProps> = (props) => {
  const { children, note, showOriginal } = props;
  const shouldReduceMotion = useReducedMotion();
  const noteRef = useRef<HTMLDivElement>(null);
  const originalRef = useRef<HTMLDivElement>(null);
  const [height, setHeight] = useState<number | "auto">("auto");
  const transition = {
    duration: shouldReduceMotion ? 0 : 0.18,
    ease: "easeOut",
  } as const;

  useLayoutEffect(() => {
    const node = showOriginal ? originalRef.current : noteRef.current;

    if (!node) return;

    const updateHeight = () => {
      const nextHeight = node.getBoundingClientRect().height;

      setHeight((current) => {
        return current === nextHeight ? current : nextHeight;
      });
    };

    updateHeight();

    if (typeof ResizeObserver === "undefined") return;

    const observer = new ResizeObserver(updateHeight);
    observer.observe(node);

    return () => {
      observer.disconnect();
    };
  }, [showOriginal]);

  return (
    <motion.div
      animate={{ height }}
      className="relative overflow-hidden"
      initial={false}
      transition={transition}
    >
      <motion.div
        animate={{ opacity: showOriginal ? 0 : 1 }}
        aria-hidden={showOriginal}
        className={cn("absolute inset-x-0 top-0", {
          "pointer-events-none invisible": showOriginal,
        })}
        initial={false}
        ref={noteRef}
        transition={transition}
      >
        <NoteAnnotation note={note} />
      </motion.div>

      <motion.div
        animate={{ opacity: showOriginal ? 1 : 0 }}
        aria-hidden={!showOriginal}
        className={cn("absolute inset-x-0 top-0", {
          "pointer-events-none invisible": !showOriginal,
        })}
        initial={false}
        ref={originalRef}
        transition={transition}
      >
        {children}
      </motion.div>
    </motion.div>
  );
};

export default NoteContentSwitcher;
