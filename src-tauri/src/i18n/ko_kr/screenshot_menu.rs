use crate::i18n::keys::ScreenshotMenuKey as Key;

pub fn label(key: Key) -> &'static str {
    match key {
        Key::Copy => "복사",
        Key::Save => "저장",
        Key::Close => "닫기",
        Key::SaveDialogTitle => "스크린샷 저장",
    }
}
