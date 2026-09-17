pub use crate::i18n::keys::ScreenshotMenuKey as Key;

use crate::settings::Language;

/// Returns labels for screenshot pin menus and save dialogs.
pub fn label(lang: Language, key: Key) -> &'static str {
    match lang {
        Language::KoKR => crate::i18n::ko_kr::screenshot_menu::label(key),
        Language::EnUS => crate::i18n::en_us::screenshot_menu::label(key),
    }
}
