import { Popover as AntdPopover, type PopoverProps } from "antd";
import type { FC } from "react";
import { useState } from "react";
import Tooltip, {
  type OverlayTooltipConfig,
  resolveOverlayTooltipProps,
} from "@/components/Tooltip";

export interface AppPopoverProps extends PopoverProps {
  tooltip?: OverlayTooltipConfig | false;
}

const renderPopoverTrigger = (
  children: PopoverProps["children"],
  tooltip: OverlayTooltipConfig | false | undefined,
  open: boolean,
): PopoverProps["children"] => {
  if (tooltip === false || tooltip === null || tooltip === void 0) {
    return children;
  }

  const tooltipProps = resolveOverlayTooltipProps(tooltip);

  return (
    <Tooltip {...tooltipProps} open={open ? false : tooltipProps.open}>
      {children}
    </Tooltip>
  );
};

const Popover: FC<AppPopoverProps> = (props) => {
  const { align, children, onOpenChange, open, tooltip, ...rest } = props;
  const [innerOpen, setInnerOpen] = useState(false);
  const mergedOpen = open ?? innerOpen;

  const handleOpenChange: NonNullable<PopoverProps["onOpenChange"]> = (
    nextOpen,
  ) => {
    setInnerOpen(nextOpen);
    onOpenChange?.(nextOpen);
  };

  return (
    <AntdPopover
      align={
        align ?? { overflow: { adjustY: true, shiftX: true, shiftY: true } }
      }
      onOpenChange={handleOpenChange}
      open={open}
      {...rest}
    >
      {renderPopoverTrigger(children, tooltip, mergedOpen)}
    </AntdPopover>
  );
};

export default Popover;
