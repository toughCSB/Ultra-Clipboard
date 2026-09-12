use crate::i18n::keys::TrayKey as Key;

pub fn label(key: Key) -> &'static str {
    match key {
        Key::Preference => "설정",
        Key::StartListening => "기록 시작",
        Key::StopListening => "기록 중지",
        Key::Version => "버전",
        Key::Relaunch => "다시 시작",
        Key::Exit => "종료",
    }
}
