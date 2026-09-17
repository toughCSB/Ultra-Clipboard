import type {
  ChangeEvent,
  FC,
  KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { useLayoutEffect, useRef } from "react";
import type { TextStyle } from "../model/shapes";
import type { Viewport } from "../model/viewport";
import {
  contrastColor,
  measureTextBox,
  TEXT_FONT_FAMILY,
  TEXT_LINE_HEIGHT,
  textPadding,
} from "../render/text";

export interface TextEditState {
  color: string;
  fontSize: number;
  /** Existing annotation being edited, or `null` for new text. */
  id: string | null;
  style: TextStyle;
  text: string;
  x: number;
  y: number;
}

interface TextEditorProps {
  edit: TextEditState;
  ratio: number;
  viewport: Viewport;
  onChange: (text: string) => void;
  onCommit: () => void;
}

/** Native textarea placed over the canvas so IME composition works for text annotations. */
const TextEditor: FC<TextEditorProps> = (props) => {
  const { edit, ratio, viewport, onChange, onCommit } = props;
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const cssScale = viewport.zoom / ratio;
  const padding = textPadding(edit.fontSize, edit.style);
  const size = measureTextBox(edit.text || " ", edit.fontSize, edit.style);

  useLayoutEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;

    textarea.focus({ preventScroll: true });
    textarea.setSelectionRange(textarea.value.length, textarea.value.length);
  }, []);

  const handleChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
    onChange(event.target.value);
  };

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    event.stopPropagation();
    if (event.nativeEvent.isComposing) return;

    if (event.key === "Escape" || (event.key === "Enter" && !event.shiftKey)) {
      event.preventDefault();
      onCommit();
    }
  };

  const background = edit.style === "background";

  return (
    <textarea
      className="absolute m-0 box-border resize-none overflow-hidden whitespace-pre border-none p-0 outline-dashed outline-2 outline-teal-500"
      onBlur={onCommit}
      onChange={handleChange}
      onKeyDown={handleKeyDown}
      onKeyUp={(event) => event.stopPropagation()}
      ref={textareaRef}
      spellCheck={false}
      style={{
        backgroundColor: background ? edit.color : "transparent",
        borderRadius: background ? edit.fontSize * 0.25 * cssScale : 0,
        caretColor: background ? contrastColor(edit.color) : edit.color,
        color: background ? contrastColor(edit.color) : edit.color,
        fontFamily: TEXT_FONT_FAMILY,
        fontSize: edit.fontSize * cssScale,
        fontWeight: 600,
        height: size.height * cssScale,
        left: (edit.x * viewport.zoom + viewport.offsetX) / ratio,
        lineHeight: TEXT_LINE_HEIGHT,
        padding: `${padding.y * cssScale}px ${padding.x * cssScale}px`,
        paintOrder: "stroke fill",
        top: (edit.y * viewport.zoom + viewport.offsetY) / ratio,
        WebkitTextStroke:
          edit.style === "outline"
            ? `${edit.fontSize * 0.18 * cssScale}px ${contrastColor(edit.color)}`
            : undefined,
        width: (size.width + edit.fontSize * 0.5) * cssScale,
      }}
      value={edit.text}
    />
  );
};

export default TextEditor;
