use crate::i18n::keys::TrayKey as Key;

pub fn label(key: Key) -> &'static str {
    match key {
        Key::Preference => "Preference",
        Key::StartListening => "Start Listening",
        Key::StopListening => "Stop Listening",
        Key::Version => "Version",
        Key::Relaunch => "Relaunch",
        Key::Exit => "Exit",
    }
}
