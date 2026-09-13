import { Tooltip as AntdTooltip, type TooltipProps } from "antd";
import type { FC } from "react";
import { isValidElement } from "react";

export type OverlayTooltipConfig =
  | TooltipProps["title"]
  | Omit<TooltipProps, "children">;

export const isOverlayTooltipProps = (
  tooltip: OverlayTooltipConfig,
): tooltip is Omit<TooltipProps, "children"> => {
  if (tooltip === null) return false;
  if (Array.isArray(tooltip)) return false;
  if (isValidElement(tooltip)) return false;

  return typeof tooltip === "object";
};

export const resolveOverlayTooltipProps = (
  tooltip: OverlayTooltipConfig,
): Omit<TooltipProps, "children"> => {
  if (isOverlayTooltipProps(tooltip)) return tooltip;

  return { title: tooltip };
};

const Tooltip: FC<TooltipProps> = (props) => {
  const { align, ...rest } = props;

  return (
    <AntdTooltip
      align={
        align ?? { overflow: { adjustY: true, shiftX: true, shiftY: true } }
      }
      {...rest}
    />
  );
};

export default Tooltip;
