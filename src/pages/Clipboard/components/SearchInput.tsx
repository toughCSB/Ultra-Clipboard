import { Input, type InputProps, type InputRef } from "antd";
import type { ChangeEvent, CompositionEvent, FC } from "react";
import { useCallback, useEffect, useRef } from "react";
import KeyHint from "@/components/KeyHint";
import { prepareClipboardWindowEditableFocus } from "@/hooks/useClipboardWindowEditableFocus";

interface SearchInputProps extends Omit<InputProps, "prefix"> {
  blurToken?: number;
  clearToken?: number;
  focusToken?: number;
}
/** Search input with shortcut focus and IME-safe change handling. */
const SearchInput: FC<SearchInputProps> = (props) => {
  const {
    blurToken = 0,
    clearToken = 0,
    focusToken = 0,
    onChange,
    onCompositionStart,
    onCompositionEnd,
    ...rest
  } = props;

  const inputRef = useRef<InputRef>(null);
  const composingRef = useRef(false);

  /** Focus and select the existing query for immediate replacement. */
  const focusSearch = useCallback(async () => {
    if (!inputRef.current) return;

    await prepareClipboardWindowEditableFocus();
    inputRef.current?.focus({ cursor: "all" });
  }, []);

  useEffect(() => {
    if (blurToken <= 0) return;

    inputRef.current?.blur();
  }, [blurToken]);

  useEffect(() => {
    if (focusToken <= 0) return;

    const frame = requestAnimationFrame(() => {
      void focusSearch();
    });

    return () => {
      cancelAnimationFrame(frame);
    };
  }, [focusToken, focusSearch]);

  const handleChange = (event: ChangeEvent<HTMLInputElement>) => {
    if (composingRef.current) return;

    onChange?.(event);
  };

  const handleCompositionStart = (
    event: CompositionEvent<HTMLInputElement>,
  ) => {
    composingRef.current = true;

    onCompositionStart?.(event);
  };

  const handleCompositionEnd = (event: CompositionEvent<HTMLInputElement>) => {
    composingRef.current = false;

    onCompositionEnd?.(event);

    // The final input event was suppressed during composition, so emit it here.
    onChange?.(event as unknown as ChangeEvent<HTMLInputElement>);
  };

  return (
    <Input
      autoCapitalize="off"
      autoCorrect="off"
      data-allow-global-keyboard="true"
      key={clearToken}
      onChange={handleChange}
      onCompositionEnd={handleCompositionEnd}
      onCompositionStart={handleCompositionStart}
      prefix={
        <KeyHint
          hintKey="F"
          iconName="i-lucide:search"
          onKeyPress={focusSearch}
        />
      }
      ref={inputRef}
      spellCheck={false}
      {...rest}
    />
  );
};

export default SearchInput;
