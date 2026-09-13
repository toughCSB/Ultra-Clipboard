import type { ButtonProps } from "antd";
import { Button } from "antd";
import type { FC } from "react";
import { cn } from "@/utils/cn";

interface CustomIconButtonProps extends ButtonProps {
  iconClassName?: string;
}

const CUSTOM_ICON_CLASS_NAME =
  "inline-flex shrink-0 items-center justify-center leading-none";

const CustomIconButton: FC<CustomIconButtonProps> = (props) => {
  const { classNames, iconClassName, ...rest } = props;
  const iconSlotClassName = cn(CUSTOM_ICON_CLASS_NAME, iconClassName);

  return (
    <Button
      {...rest}
      classNames={
        typeof classNames === "function"
          ? (context) => {
              const nextClassNames = classNames(context);

              return {
                ...nextClassNames,
                icon: cn(nextClassNames?.icon, iconSlotClassName),
              };
            }
          : {
              ...classNames,
              icon: cn(classNames?.icon, iconSlotClassName),
            }
      }
    />
  );
};

export default CustomIconButton;
