//! Which actions a stored clipboard item offers. Rust owns the rule; the
//! frontend only renders `available_actions`.

use crate::db::models::{ClipboardAction, ClipboardItem, ClipboardKind, ClipboardSubKind};
use crate::settings::Content;

/// `in_favorite_view` is true when the caller lists or deletes from the
/// favorite range; favorites may be limited to deletion there only.
pub fn can_delete(item: &ClipboardItem, content: &Content, in_favorite_view: bool) -> bool {
    if item.is_pinned && !content.delete_pinned_items {
        return false;
    }

    if !item.is_favorite {
        return true;
    }

    if !content.delete_favorite_items {
        return false;
    }

    !content.delete_favorite_items_only_in_favorite_group || in_favorite_view
}

pub fn available_actions(
    item: &ClipboardItem,
    content: &Content,
    in_favorite_view: bool,
) -> Vec<ClipboardAction> {
    let mut actions = Vec::with_capacity(10);

    actions.push(ClipboardAction::Paste);

    match item.kind {
        ClipboardKind::Text => actions.push(ClipboardAction::PasteAsPlainText),
        ClipboardKind::Files => actions.push(ClipboardAction::PasteAsPath),
        ClipboardKind::Image => {}
    }

    actions.push(ClipboardAction::Copy);
    if item.kind == ClipboardKind::Image {
        actions.push(ClipboardAction::SaveImage);
    }

    match item.sub_kind {
        Some(ClipboardSubKind::Url) => actions.push(ClipboardAction::OpenLink),
        Some(ClipboardSubKind::Email) => actions.push(ClipboardAction::SendEmail),
        _ => {}
    }

    let can_reveal =
        item.kind == ClipboardKind::Files || item.sub_kind == Some(ClipboardSubKind::Path);
    if can_reveal {
        #[cfg(target_os = "macos")]
        actions.push(ClipboardAction::RevealInFinder);
        #[cfg(target_os = "windows")]
        actions.push(ClipboardAction::RevealInExplorer);
    }

    actions.push(ClipboardAction::ToggleFavorite);
    actions.push(ClipboardAction::TogglePinned);
    actions.push(ClipboardAction::EditNote);
    if can_delete(item, content, in_favorite_view) {
        actions.push(ClipboardAction::Delete);
    }

    actions
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::db::models::Platform;

    fn item(is_favorite: bool, is_pinned: bool) -> ClipboardItem {
        ClipboardItem {
            id: "item".to_owned(),
            kind: ClipboardKind::Text,
            sub_kind: None,
            group_id: None,
            source_app_id: None,
            content: "text".to_owned(),
            content_hash: "hash".to_owned(),
            search_text: None,
            summary: None,
            file_types: None,
            size: None,
            width: None,
            height: None,
            use_count: 1,
            is_favorite,
            is_pinned,
            is_sensitive: false,
            platform: Platform::Windows,
            note: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source_app_name: None,
            source_app_icon_file: None,
            source_app_icon_path: None,
            image_thumbnail_path: None,
            file_entries: None,
            files_preview_kind: None,
            available_actions: Vec::new(),
            color_preview: None,
            display_created_at: String::new(),
        }
    }

    #[test]
    fn plain_item_is_deletable_with_default_settings() {
        assert!(can_delete(&item(false, false), &Content::default(), false));
    }

    #[test]
    fn pinned_item_needs_delete_pinned_items() {
        let mut content = Content::default();

        assert!(!can_delete(&item(false, true), &content, false));

        content.delete_pinned_items = true;
        assert!(can_delete(&item(false, true), &content, false));
    }

    #[test]
    fn favorite_item_needs_delete_favorite_items() {
        let mut content = Content {
            delete_favorite_items_only_in_favorite_group: false,
            ..Content::default()
        };

        assert!(!can_delete(&item(true, false), &content, true));

        content.delete_favorite_items = true;
        assert!(can_delete(&item(true, false), &content, false));
    }

    #[test]
    fn pinned_favorite_needs_both_delete_settings() {
        let pinned_only = Content {
            delete_pinned_items: true,
            delete_favorite_items_only_in_favorite_group: false,
            ..Content::default()
        };
        let both = Content {
            delete_favorite_items: true,
            ..pinned_only.clone()
        };

        assert!(!can_delete(&item(true, true), &pinned_only, true));
        assert!(can_delete(&item(true, true), &both, false));
    }

    #[test]
    fn image_actions_include_save_image() {
        let mut image = item(false, false);
        image.kind = ClipboardKind::Image;

        let actions = available_actions(&image, &Content::default(), false);

        assert!(actions.contains(&ClipboardAction::SaveImage));
    }

    #[test]
    fn text_actions_do_not_include_save_image() {
        let actions = available_actions(&item(false, false), &Content::default(), false);

        assert!(!actions.contains(&ClipboardAction::SaveImage));
    }

    #[test]
    fn actions_offer_delete_only_when_the_item_is_deletable() {
        let content = Content::default();

        assert!(available_actions(&item(false, false), &content, false)
            .contains(&ClipboardAction::Delete));
        assert!(!available_actions(&item(true, false), &content, true)
            .contains(&ClipboardAction::Delete));
    }

    #[test]
    fn favorite_group_rule_limits_favorite_delete_to_favorite_view() {
        let content = Content {
            delete_favorite_items: true,
            delete_favorite_items_only_in_favorite_group: true,
            ..Content::default()
        };

        assert!(!can_delete(&item(true, false), &content, false));
        assert!(can_delete(&item(true, false), &content, true));
        assert!(can_delete(&item(false, false), &content, false));
    }
}
