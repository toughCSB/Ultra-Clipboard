pub use crate::i18n::keys::ClipboardMenuKey as Key;

use crate::settings::Language;

/// Returns the clipboard item context-menu label.
pub fn label(lang: Language, key: Key) -> &'static str {
    match lang {
        Language::KoKR => crate::i18n::ko_kr::clipboard_menu::label(key),
        Language::EnUS => crate::i18n::en_us::clipboard_menu::label(key),
    }
}
