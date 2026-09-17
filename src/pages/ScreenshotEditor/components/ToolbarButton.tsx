import { Button } from "antd";
import type {
  FC,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
} from "react";
import Tooltip from "@/components/Tooltip";
import { cn } from "@/utils/cn";

interface ToolbarButtonProps {
  active?: boolean;
  className?: string;
  danger?: boolean;
  disabled?: boolean;
  icon: string;
  /** Tints the icon glyph itself, independent of the active/hover background. */
  iconColor?: string;
  loading?: boolean;
  shortcut?: string;
  title: string;
  onClick?: (event: ReactMouseEvent<HTMLElement>) => void;
  onDoubleClick?: (event: ReactMouseEvent<HTMLElement>) => void;
  onPointerDown?: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerEnter?: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerMove?: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerUp?: (event: ReactPointerEvent<HTMLElement>) => void;
}

/** Square icon button used by the screenshot editor toolbar. */
const ToolbarButton: FC<ToolbarButtonProps> = (props) => {
  const {
    active = false,
    className,
    danger = false,
    icon,
    iconColor,
    shortcut,
    title,
    ...rest
  } = props;
  const tooltip = shortcut ? `${title} (${shortcut})` : title;

  return (
    <Tooltip mouseEnterDelay={0.4} placement="bottom" title={tooltip}>
      <Button
        aria-label={title}
        aria-pressed={active}
        className={cn(
          "h-9 w-9 shrink-0 rounded-2 p-0",
          {
            "bg-ant-fill-secondary": active,
            "hover:bg-ant-error! hover:text-white!": danger,
          },
          className,
        )}
        icon={
          <span
            className={cn(icon, "text-lg")}
            style={iconColor ? { color: iconColor } : undefined}
          />
        }
        type="text"
        {...rest}
      />
    </Tooltip>
  );
};

export default ToolbarButton;
