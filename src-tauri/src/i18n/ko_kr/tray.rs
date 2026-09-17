use crate::i18n::keys::TrayKey as Key;

pub fn label(key: Key) -> &'static str {
    match key {
        Key::CaptureArea => "영역 캡처",
        Key::CaptureFullscreen => "전체 화면 캡처",
        Key::CaptureWindow => "창 캡처",
        Key::CaptureRepeat => "마지막 영역 다시 캡처",
        Key::CaptureDelayed => "3초 후 영역 캡처",
        Key::Preference => "설정",
        Key::StartListening => "기록 시작",
        Key::StopListening => "기록 중지",
        Key::Version => "버전",
        Key::Relaunch => "다시 시작",
        Key::Exit => "종료",
    }
}
