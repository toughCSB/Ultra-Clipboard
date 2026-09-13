import DOMPurify from "dompurify";
import type { CSSProperties, FC } from "react";
import type { ClipboardGroupIcon as ClipboardGroupIconValue } from "@/types/clipboard";
import { cn } from "@/utils/cn";

const GROUP_ICON_MASK_VARIABLE = "--clipboard-group-icon-mask";

type ClipboardGroupIconMaskStyle = CSSProperties & {
  [GROUP_ICON_MASK_VARIABLE]: string;
};

interface ClipboardGroupIconProps {
  className?: string;
  disabled?: boolean;
  icon: ClipboardGroupIconValue;
  inheritColor?: boolean;
  selected?: boolean;
}

const isCustomSvgIcon = (icon: ClipboardGroupIconValue) => {
  return icon.trimStart().startsWith("<svg");
};

const sanitizeGroupSvg = (icon: ClipboardGroupIconValue) => {
  return DOMPurify.sanitize(icon, {
    FORBID_ATTR: ["style"],
    FORBID_TAGS: ["foreignObject", "script"],
    USE_PROFILES: { svg: true },
  });
};

const buildGroupSvgMaskStyle = (
  icon: ClipboardGroupIconValue,
): ClipboardGroupIconMaskStyle => {
  const svg = sanitizeGroupSvg(icon);
  const mask = `url("data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}")`;

  return { [GROUP_ICON_MASK_VARIABLE]: mask };
};

const ClipboardGroupIcon: FC<ClipboardGroupIconProps> = (props) => {
  const {
    className,
    disabled = false,
    icon,
    inheritColor = false,
    selected = false,
  } = props;

  const stateClassName = cn({
    "text-ant-disabled": disabled && !inheritColor,
    "text-ant-light-solid": selected && !disabled && !inheritColor,
    "text-ant-secondary": !selected && !disabled && !inheritColor,
  });

  if (isCustomSvgIcon(icon)) {
    return (
      <i
        aria-hidden
        className={cn(
          "inline-block shrink-0 bg-current text-base [-webkit-mask-image:var(--clipboard-group-icon-mask)] [-webkit-mask-position:center] [-webkit-mask-repeat:no-repeat] [-webkit-mask-size:contain] [height:1em] [mask-image:var(--clipboard-group-icon-mask)] [mask-position:center] [mask-repeat:no-repeat] [mask-size:contain] [width:1em]",
          stateClassName,
          className,
        )}
        style={buildGroupSvgMaskStyle(icon)}
      />
    );
  }

  return (
    <i
      aria-hidden
      className={cn(
        icon.trim(),
        "inline-block shrink-0 text-base",
        stateClassName,
        className,
      )}
    />
  );
};

export default ClipboardGroupIcon;
