import type { FC, ReactNode } from "react";
import { cn } from "@/utils/cn";

interface ToolbarInfoProps {
  caption: string;
  className?: string;
  leading?: ReactNode;
  value: string;
  onClick?: () => void;
}

/** Two-line value and caption shown on the right side of the editor toolbar. */
const ToolbarInfo: FC<ToolbarInfoProps> = (props) => {
  const { caption, className, leading, value, onClick } = props;
  const content = (
    <>
      {leading}
      <span className="flex min-w-0 flex-col items-start leading-tight">
        <span className="font-semibold text-xs tabular-nums">{value}</span>
        <span className="text-ant-secondary text-xs">{caption}</span>
      </span>
    </>
  );

  if (onClick) {
    return (
      <button
        className={cn(
          "flex h-10 shrink-0 cursor-pointer items-center gap-2 rounded-2 border-none bg-transparent px-2 text-ant-text hover:bg-ant-fill-secondary",
          className,
        )}
        onClick={onClick}
        type="button"
      >
        {content}
      </button>
    );
  }

  return (
    <div
      className={cn("flex h-10 shrink-0 items-center gap-2 px-2", className)}
    >
      {content}
    </div>
  );
};

export default ToolbarInfo;
