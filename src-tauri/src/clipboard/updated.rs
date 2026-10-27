//! The `clipboard://updated` payload. `List.tsx` branches on which key is
//! present, so each variant keeps its JSON shape exactly.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::core::events;
use crate::db::models::ClipboardKind;

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ClipboardUpdated<'a> {
    /// A new, reused, or synced item.
    Captured {
        id: &'a str,
        kind: ClipboardKind,
        deduplicated: bool,
    },
    /// Items removed by history cleanup or a clear.
    Cleanup {
        #[serde(rename = "cleanup")]
        removed: u64,
    },
    /// A backup, legacy, or storage import finished.
    Imported { imported: bool },
}

pub fn emit_clipboard_updated(app: &AppHandle, update: ClipboardUpdated<'_>) {
    if let Err(err) = app.emit(events::CLIPBOARD_UPDATED, &update) {
        log::warn!(
            "emit {} {update:?} failed: {err}",
            events::CLIPBOARD_UPDATED
        );
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn captured_keeps_id_kind_and_deduplicated() {
        let update = ClipboardUpdated::Captured {
            id: "item-1",
            kind: ClipboardKind::Image,
            deduplicated: true,
        };

        assert_eq!(
            serde_json::to_value(update).unwrap(),
            json!({ "id": "item-1", "kind": "image", "deduplicated": true })
        );
    }

    #[test]
    fn cleanup_sends_the_removed_count_as_cleanup() {
        let update = ClipboardUpdated::Cleanup { removed: 3 };

        assert_eq!(
            serde_json::to_value(update).unwrap(),
            json!({ "cleanup": 3 })
        );
    }

    #[test]
    fn imported_sends_only_the_imported_flag() {
        let update = ClipboardUpdated::Imported { imported: true };

        assert_eq!(
            serde_json::to_value(update).unwrap(),
            json!({ "imported": true })
        );
    }
}
