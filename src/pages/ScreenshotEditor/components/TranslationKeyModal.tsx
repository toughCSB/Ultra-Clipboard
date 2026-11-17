import { Button, Input, Modal } from "antd";
import type { ChangeEvent, FC } from "react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ScreenshotTranslationStatus } from "@/commands";

interface TranslationKeyModalProps {
  busy: boolean;
  configured: boolean;
  model: string | null;
  open: boolean;
  source: ScreenshotTranslationStatus["source"];
  onCancel: () => void;
  onRemove: () => void;
  onSave: (key: string) => void;
}

/** The key only lives in this input until the save action takes its value. */
const TranslationKeyModal: FC<TranslationKeyModalProps> = (props) => {
  const { t } = useTranslation("screenshot");
  const { busy, configured, model, open, source, onCancel, onRemove, onSave } =
    props;
  const [keyValue, setKeyValue] = useState("");
  const environmentKey = source === "environment";

  useEffect(() => {
    if (!open) setKeyValue("");
  }, [open]);

  const handleChange = (event: ChangeEvent<HTMLInputElement>) => {
    setKeyValue(event.target.value);
  };

  const handleSave = () => {
    const key = keyValue.trim();
    if (!key || busy || environmentKey) return;

    setKeyValue("");
    onSave(key);
  };

  const handleCancel = () => {
    setKeyValue("");
    onCancel();
  };

  return (
    <Modal
      footer={
        <div className="flex items-center justify-between gap-2">
          {configured ? (
            <Button danger disabled={busy || environmentKey} onClick={onRemove}>
              {t("editor.translation.removeKey")}
            </Button>
          ) : (
            <span />
          )}
          <div className="flex gap-2">
            <Button disabled={busy} onClick={handleCancel}>
              {t("editor.translation.cancel")}
            </Button>
            <Button
              disabled={environmentKey || !keyValue.trim()}
              loading={busy}
              onClick={handleSave}
              type="primary"
            >
              {t("editor.translation.saveKey")}
            </Button>
          </div>
        </div>
      }
      onCancel={handleCancel}
      open={open}
      title={t("editor.translation.keyTitle")}
    >
      <p className="mb-3 text-ant-secondary text-sm">
        {t(
          environmentKey
            ? "editor.translation.environmentExplanation"
            : "editor.translation.keyExplanation",
        )}
      </p>
      {configured && (
        <p className="mb-2 text-ant-secondary text-xs">
          {t(
            environmentKey
              ? "editor.translation.environmentConfigured"
              : "editor.translation.keyConfigured",
            { model: model ?? "" },
          )}
        </p>
      )}
      <Input.Password
        aria-label={t("editor.translation.keyLabel")}
        autoComplete="off"
        disabled={environmentKey}
        onChange={handleChange}
        onPressEnter={handleSave}
        placeholder={t("editor.translation.keyPlaceholder")}
        value={keyValue}
      />
    </Modal>
  );
};

export default TranslationKeyModal;
