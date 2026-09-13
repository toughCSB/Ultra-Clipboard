import { Input, type InputRef } from "antd";
import { AnimatePresence, motion } from "motion/react";
import {
  type FC,
  type KeyboardEvent,
  type MouseEvent,
  useEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { resumeGlobalShortcuts, suspendGlobalShortcuts } from "@/commands";
import { cn } from "@/utils/cn";
import { getMessageApi } from "@/utils/feedback";
import { log } from "@/utils/log";
import {
  buildShortcutFromEvent,
  buildShortcutPreviewFromEvent,
  formatRecordedShortcut,
  isRecordableShortcut,
  normalizeShortcutValue,
  resolveShortcutEventKey,
} from "@/utils/shortcut";

export interface ShortcutRecorderConflict {
  label: string;
  value: string;
}

interface ShortcutRecorderProps {
  className?: string;
  conflicts?: readonly ShortcutRecorderConflict[];
  disabled?: boolean;
  onChange?: (value: string) => void | Promise<void>;
  placeholder?: string;
  value?: string;
}

const ShortcutRecorder: FC<ShortcutRecorderProps> = (props) => {
  const {
    className,
    conflicts = [],
    disabled = false,
    onChange,
    placeholder,
    value = "",
  } = props;
  const { t } = useTranslation("common");

  const inputRef = useRef<InputRef>(null);
  const committedValueRef = useRef(value);
  const draftValueRef = useRef(value);
  const recordingRef = useRef(false);
  const shortcutsSuspendedRef = useRef(false);
  const suspendPromiseRef = useRef<Promise<boolean> | null>(null);
  const [draft, setDraft] = useState(value);
  const [recording, setRecording] = useState(false);

  useEffect(() => {
    committedValueRef.current = value;

    if (recording) return;

    draftValueRef.current = value;
    setDraft(value);
  }, [recording, value]);

  useEffect(() => {
    return () => {
      if (!shortcutsSuspendedRef.current && !suspendPromiseRef.current) return;

      void resumeShortcutsAfterUnmount(
        suspendPromiseRef.current,
        shortcutsSuspendedRef.current,
      );
    };
  }, []);

  const displayValue = formatRecordedShortcut(draft);
  const showClearIcon = draft !== "" && !recording && !disabled;
  const inputPlaceholder = recording
    ? t("shortcutRecorder.press")
    : draft
      ? placeholder
      : t("shortcutRecorder.click");
  const fieldText = displayValue || inputPlaceholder;

  const commit = async (nextValue: string) => {
    if (nextValue === committedValueRef.current) return;

    committedValueRef.current = nextValue;
    await onChange?.(nextValue);
  };

  const setDraftValue = (nextValue: string) => {
    draftValueRef.current = nextValue;
    setDraft(nextValue);
  };

  const setRecordingState = (nextRecording: boolean) => {
    recordingRef.current = nextRecording;
    setRecording(nextRecording);
  };

  const findShortcutConflict = (nextValue: string) => {
    const normalizedNextValue = normalizeShortcutValue(nextValue);
    if (!normalizedNextValue) return null;

    return (
      conflicts.find((conflict) => {
        return normalizeShortcutValue(conflict.value) === normalizedNextValue;
      }) ?? null
    );
  };

  const resetConflictedDraft = (
    conflict: ShortcutRecorderConflict,
    nextValue: string,
  ) => {
    getMessageApi().warning(
      t("shortcutRecorder.conflict", {
        label: conflict.label,
        shortcut: formatRecordedShortcut(nextValue),
      }),
    );
    setDraftValue("");
  };

  const runSuspendShortcuts = async () => {
    try {
      await suspendGlobalShortcuts();
      shortcutsSuspendedRef.current = true;
      suspendPromiseRef.current = null;

      return true;
    } catch (error) {
      log.error("suspend global shortcuts for recorder failed", error);
    }

    suspendPromiseRef.current = null;

    return false;
  };

  const resumeShortcuts = async () => {
    if (suspendPromiseRef.current) {
      await suspendPromiseRef.current;
    }
    if (!shortcutsSuspendedRef.current) return;

    shortcutsSuspendedRef.current = false;
    await resumeGlobalShortcuts();
  };

  const suspendShortcuts = () => {
    if (shortcutsSuspendedRef.current) return;
    if (suspendPromiseRef.current) return;

    suspendPromiseRef.current = runSuspendShortcuts();
  };

  const clearShortcut = async () => {
    setDraftValue("");
    setRecordingState(false);
    await commit("");
    await resumeShortcuts();
    inputRef.current?.blur();
  };

  const handleFocus = () => {
    if (disabled) return;
    if (recording) return;

    setDraftValue("");
    setRecordingState(true);
    suspendShortcuts();
  };

  const handleBlur = async () => {
    if (!recordingRef.current) return;

    const draftValue = draftValueRef.current;
    const nextValue = isRecordableShortcut(draftValue) ? draftValue : value;
    const conflict = findShortcutConflict(nextValue);
    if (conflict) {
      resetConflictedDraft(conflict, nextValue);
      setRecordingState(false);
      setDraftValue(value);
      await resumeShortcuts();

      return;
    }

    setDraftValue(nextValue);
    setRecordingState(false);
    await commit(nextValue);
    await resumeShortcuts();
  };

  const handleKeyDown = async (event: KeyboardEvent<HTMLFieldSetElement>) => {
    if (disabled) return;

    event.preventDefault();
    event.stopPropagation();

    if (suspendPromiseRef.current) {
      await suspendPromiseRef.current;
    }

    if (isClearKey(event)) {
      await clearShortcut();

      return;
    }

    if (event.key === "Escape") {
      await resumeShortcuts();
      inputRef.current?.blur();

      return;
    }

    const previewValue = buildShortcutPreviewFromEvent(event);
    if (!previewValue) return;

    setDraftValue(previewValue);

    const nextValue = buildShortcutFromEvent(event);
    if (!nextValue) return;
    if (!isRecordableShortcut(nextValue)) return;
    const conflict = findShortcutConflict(nextValue);
    if (conflict) {
      resetConflictedDraft(conflict, nextValue);

      return;
    }

    setRecordingState(false);
    await commit(nextValue);
    await resumeShortcuts();
    inputRef.current?.blur();
  };

  const handleKeyUp = (event: KeyboardEvent<HTMLFieldSetElement>) => {
    if (disabled) return;

    event.preventDefault();
    event.stopPropagation();

    if (!recordingRef.current) return;

    const releasedKey = resolveShortcutEventKey(event);
    if (!releasedKey) return;

    const nextValue = removeShortcutKey(
      draftValueRef.current,
      releasedKey.shortcutKey,
    );

    setDraftValue(nextValue);
  };

  const handleClearClick = async () => {
    await clearShortcut();
  };

  return (
    <fieldset
      className={cn(
        "group/shortcut-recorder relative w-fit min-w-32 border-0 p-0",
        className,
      )}
      disabled={disabled}
      onBlur={handleBlur}
      onKeyDown={handleKeyDown}
      onKeyUp={handleKeyUp}
    >
      <span
        aria-hidden
        className="invisible block h-8 min-w-32 whitespace-pre px-3 text-center font-bold text-sm"
      >
        {displayValue}
      </span>
      <Input
        className="absolute inset-0 w-full min-w-0"
        classNames={{
          input: "text-transparent caret-transparent",
        }}
        disabled={disabled}
        onFocus={handleFocus}
        readOnly
        ref={inputRef}
      />
      <AnimatePresence initial={false} mode="wait">
        <motion.span
          animate={{ opacity: 1, scale: 1 }}
          className={cn(
            "pointer-events-none absolute inset-0 flex items-center justify-center whitespace-pre px-3 text-center text-sm",
            draft === "" ? "text-ant-tertiary" : "font-bold text-ant-primary",
          )}
          exit={{ opacity: 0, scale: 0.98 }}
          initial={{ opacity: 0, scale: 0.98 }}
          key={fieldText}
          transition={{ duration: 0.1, ease: "easeOut" }}
        >
          {fieldText}
        </motion.span>
      </AnimatePresence>
      {showClearIcon && (
        <button
          aria-label={t("shortcutRecorder.clear")}
          className="pointer-events-none absolute top-1/2 right-2 z-1 inline-flex size-4 -translate-y-1/2 cursor-pointer items-center justify-center border-0 bg-transparent p-0 text-ant-tertiary opacity-0 transition hover:text-ant-error group-hover/shortcut-recorder:pointer-events-auto group-hover/shortcut-recorder:opacity-100"
          data-shortcut-recorder-clear
          onClick={handleClearClick}
          onMouseDown={preventClearMouseDown}
          type="button"
        >
          <i className="i-lucide:circle-x size-3.5" />
        </button>
      )}
    </fieldset>
  );
};

export default ShortcutRecorder;

function isClearKey(event: KeyboardEvent<HTMLFieldSetElement>) {
  if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) {
    return false;
  }

  return event.key === "Backspace" || event.key === "Delete";
}

function preventClearMouseDown(event: MouseEvent<HTMLButtonElement>) {
  event.preventDefault();
  event.stopPropagation();
}

async function resumeShortcutsAfterUnmount(
  suspendPromise: Promise<boolean> | null,
  suspended: boolean,
) {
  let shouldResume = suspended;
  if (suspendPromise) {
    shouldResume = await suspendPromise;
  }
  if (!shouldResume) return;

  await resumeGlobalShortcuts();
}

function removeShortcutKey(value: string, shortcutKey: string) {
  return value
    .split("+")
    .filter((key) => {
      return key !== "" && key !== shortcutKey;
    })
    .join("+");
}
