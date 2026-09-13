pub use crate::i18n::keys::TrayKey as Key;

use crate::settings::Language;

/// Returns the system tray menu label.
pub fn label(lang: Language, key: Key) -> &'static str {
    match lang {
        Language::KoKR => crate::i18n::ko_kr::tray::label(key),
        Language::EnUS => crate::i18n::en_us::tray::label(key),
    }
}
