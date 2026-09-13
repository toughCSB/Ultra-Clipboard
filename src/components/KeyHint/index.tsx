import { useEventListener, useUnmount } from "ahooks";
import { type FC, type ReactNode, useRef, useState } from "react";
import { useKeyboardEvent } from "@/hooks/useKeyboardEvent";
import { cn } from "@/utils/cn";
import { isMac, isWinClipboardWindow } from "@/utils/is";

interface KeyHintProps {
  className?: string;

  iconName?: string;

  children?: ReactNode;

  hintKey: string;

  onKeyPress?: (event: KeyboardEvent) => void;
}

const KeyHint: FC<KeyHintProps> = (props) => {
  const { hintKey, onKeyPress, iconName, children, className } = props;

  const [active, setActive] = useState(false);
  const isWindowsClipboardWindow = isWinClipboardWindow();
  const modifierPressedRef = useRef(false);
  const blurResetTimerRef = useRef(0);

  const clearBlurResetTimer = () => {
    if (blurResetTimerRef.current === 0) return;

    window.clearTimeout(blurResetTimerRef.current);
    blurResetTimerRef.current = 0;
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    const isModifierPressed = isMac ? event.metaKey : event.ctrlKey;

    if (!isModifierPressed) return;

    modifierPressedRef.current = true;
    clearBlurResetTimer();
    setActive(true);

    if (event.key.toLowerCase() !== hintKey.toLowerCase()) return;

    event.preventDefault();

    onKeyPress?.(event);
  };

  const handleKeyUp = (event: KeyboardEvent) => {
    const isModifierPressed = isMac ? event.metaKey : event.ctrlKey;

    if (isModifierPressed) return;

    modifierPressedRef.current = false;
    clearBlurResetTimer();
    setActive(false);
  };

  const handleBlur = () => {
    if (!isWindowsClipboardWindow) {
      modifierPressedRef.current = false;
      clearBlurResetTimer();
      setActive(false);

      return;
    }

    if (modifierPressedRef.current) return;

    blurResetTimerRef.current = window.setTimeout(() => {
      blurResetTimerRef.current = 0;
      if (modifierPressedRef.current) return;

      setActive(false);
    });
  };

  useKeyboardEvent("keydown", handleKeyDown);

  useKeyboardEvent("keyup", handleKeyUp);

  useEventListener("blur", handleBlur, { target: window });

  useUnmount(() => {
    clearBlurResetTimer();
  });

  return (
    <div className="relative">
      <div
        className={cn("flex items-center justify-center", {
          "opacity-0": active,
        })}
      >
        {iconName ? <i className={cn("text-base", iconName)} /> : children}
      </div>

      <span
        className={cn(
          "-translate-1/2 absolute top-1/2 left-1/2 inline-flex size-4 items-center justify-center rounded-1 bg-ant-bg-spotlight font-bold font-mono text-ant-light-solid text-xs",
          {
            "opacity-0": !active,
          },
          className,
        )}
      >
        {hintKey.toUpperCase()}
      </span>
    </div>
  );
};

export default KeyHint;
