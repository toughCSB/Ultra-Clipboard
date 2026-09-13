pub use crate::i18n::keys::CommandKey as Key;

use crate::settings::Language;

/// Returns the user-visible root-cause label for Tauri command errors.
pub fn label(lang: Language, key: Key) -> &'static str {
    match lang {
        Language::KoKR => crate::i18n::ko_kr::commands::label(key),
        Language::EnUS => crate::i18n::en_us::commands::label(key),
    }
}
