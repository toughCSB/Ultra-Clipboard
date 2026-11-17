import { Button, Modal } from "antd";
import type { FC } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { ScreenshotTranslationBlock } from "@/commands";
import CustomIconButton from "@/components/CustomIconButton";

interface TranslationControlsProps {
  blocks: readonly ScreenshotTranslationBlock[];
  failed: boolean;
  loading: boolean;
  overflowCount: number;
  showOriginal: boolean;
  onCopyText: () => void;
  onLeave: () => void;
  onManageKey: () => void;
  onRetry: () => void;
  onToggleOriginal: () => void;
}

/** Keeps editing blocked while the translated canvas is being compared or copied. */
const TranslationControls: FC<TranslationControlsProps> = (props) => {
  const { t } = useTranslation("screenshot");
  const {
    blocks,
    failed,
    loading,
    overflowCount,
    showOriginal,
    onCopyText,
    onLeave,
    onManageKey,
    onRetry,
    onToggleOriginal,
  } = props;
  const [fullTextOpen, setFullTextOpen] = useState(false);

  const closeFullText = () => {
    setFullTextOpen(false);
  };

  const openFullText = () => {
    setFullTextOpen(true);
  };

  return (
    <div className="absolute inset-0 z-10">
      <div className="absolute top-3 right-3 z-20 flex flex-wrap items-center justify-end gap-2 rounded-2 border border-ant-border bg-ant-container p-2 shadow-md">
        <span className="text-ant-secondary text-xs" role="status">
          {loading
            ? t("editor.translation.loading")
            : failed
              ? t("editor.translation.failed")
              : blocks.length === 0
                ? t("editor.translation.none")
                : t("editor.translation.ready", { count: blocks.length })}
        </span>
        {overflowCount > 0 && !loading && (
          <span className="text-ant-secondary text-xs">
            {t("editor.translation.overflow")}
          </span>
        )}
        {failed && !loading && (
          <Button onClick={onRetry} size="small">
            {t("editor.translation.retry")}
          </Button>
        )}
        <Button
          disabled={loading || blocks.length === 0}
          onClick={onToggleOriginal}
          size="small"
        >
          {t(
            showOriginal
              ? "editor.translation.showTranslation"
              : "editor.translation.showOriginal",
          )}
        </Button>
        <Button
          disabled={loading || blocks.length === 0}
          onClick={onCopyText}
          size="small"
        >
          {t("editor.translation.copyText")}
        </Button>
        <Button
          disabled={loading || blocks.length === 0}
          onClick={openFullText}
          size="small"
        >
          {t("editor.translation.showAll")}
        </Button>
        <CustomIconButton
          aria-label={t("editor.translation.manageKey")}
          icon={<span className="i-lucide:key-round" />}
          onClick={onManageKey}
          size="small"
          title={t("editor.translation.manageKey")}
        />
        <Button onClick={onLeave} size="small" type="primary">
          {t("editor.translation.back")}
        </Button>
      </div>
      <Modal
        footer={
          <Button onClick={onCopyText} type="primary">
            {t("editor.translation.copyText")}
          </Button>
        }
        onCancel={closeFullText}
        open={fullTextOpen}
        title={t("editor.translation.showAll")}
      >
        <div className="max-h-96 space-y-3 overflow-y-auto">
          {blocks.map((block) => {
            return (
              <div
                className="rounded-2 border border-ant-border p-2"
                key={block.id}
              >
                <p className="mb-1 text-ant-secondary text-xs">
                  {block.sourceText}
                </p>
                <p className="whitespace-pre-wrap text-sm">
                  {block.translatedText}
                </p>
              </div>
            );
          })}
        </div>
      </Modal>
    </div>
  );
};

export default TranslationControls;
