use crate::i18n::keys::ScreenshotMenuKey as Key;

pub fn label(key: Key) -> &'static str {
    match key {
        Key::Copy => "Copy",
        Key::Save => "Save",
        Key::Close => "Close",
        Key::SaveDialogTitle => "Save Screenshot",
    }
}
