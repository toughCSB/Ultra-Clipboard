import type { FC, ReactNode } from "react";
import { cn } from "@/utils/cn";

interface PreferenceCountTagProps {
  children: ReactNode;
  className?: string;
}

/** Shared compact count tag for settings and app counts. */
const PreferenceCountTag: FC<PreferenceCountTagProps> = (props) => {
  const { children, className } = props;

  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center rounded-full border border-ant-border-secondary bg-ant-fill-quaternary px-2 py-1 text-ant-secondary text-xs leading-none",
        className,
      )}
    >
      {children}
    </span>
  );
};

export default PreferenceCountTag;
