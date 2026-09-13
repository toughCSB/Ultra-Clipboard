import { type GetRef, Input, Modal } from "antd";
import type { ChangeEvent, FC } from "react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { updateClipboardItemNote } from "@/commands";
import type { ClipboardItem } from "@/types/clipboard";

interface NoteModalProps {
  item: ClipboardItem | null;

  onClose: () => void;

  onSaved: (id: string, note: string | null, autoFavorited: boolean) => void;
}

type TextAreaRef = GetRef<typeof Input.TextArea>;
/** Singleton note editor for the clipboard list. Empty notes become NULL in Rust. */
const NoteModal: FC<NoteModalProps> = (props) => {
  const { item, onClose, onSaved } = props;
  const { t } = useTranslation(["clipboard", "common"]);

  const [value, setValue] = useState("");
  const [saving, setSaving] = useState(false);
  const textAreaRef = useRef<TextAreaRef>(null);

  useEffect(() => {
    if (!item) return;

    setValue(item.note ?? "");
  }, [item]);

  const handleChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
    setValue(event.target.value);
  };

  const handleSave = async () => {
    if (!item) return;

    setSaving(true);

    try {
      const { note, autoFavorited } = await updateClipboardItemNote(
        item.id,
        value,
      );

      onSaved(item.id, note, autoFavorited);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  /** Focus after the modal is mounted, avoiding early Windows autoFocus. */
  const handleAfterOpenChange = (open: boolean) => {
    if (!open) return;

    requestAnimationFrame(() => {
      textAreaRef.current?.focus({ cursor: "end" });
    });
  };

  return (
    <Modal
      afterOpenChange={handleAfterOpenChange}
      confirmLoading={saving}
      destroyOnHidden
      okText={t("common:actions.save")}
      onCancel={onClose}
      onOk={handleSave}
      open={!!item}
      title={t("clipboard:note.title")}
    >
      <Input.TextArea
        autoSize={{ maxRows: 6, minRows: 3 }}
        maxLength={256}
        onChange={handleChange}
        placeholder={t("clipboard:note.placeholder")}
        ref={textAreaRef}
        value={value}
      />
    </Modal>
  );
};

export default NoteModal;
