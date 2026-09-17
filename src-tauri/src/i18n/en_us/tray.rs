use crate::i18n::keys::TrayKey as Key;

pub fn label(key: Key) -> &'static str {
    match key {
        Key::CaptureArea => "Capture Area",
        Key::CaptureFullscreen => "Capture Full Screen",
        Key::CaptureWindow => "Capture Window",
        Key::CaptureRepeat => "Repeat Last Area",
        Key::CaptureDelayed => "Capture Area in 3 Seconds",
        Key::Preference => "Preference",
        Key::StartListening => "Start Listening",
        Key::StopListening => "Stop Listening",
        Key::Version => "Version",
        Key::Relaunch => "Relaunch",
        Key::Exit => "Exit",
    }
}
