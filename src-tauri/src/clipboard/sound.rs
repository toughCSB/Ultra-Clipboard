//! 复制成功提示音。音频字节直接 `include_bytes!` 打入二进制，避免运行时文件 IO。
//! Windows 使用同步 `PlaySoundW` 播放内存 WAV；其他平台由 rodio 解码 MP3。
//! 自动提示音在短命线程中播放，不阻塞剪贴板监听和入库。

use tauri::{AppHandle, Manager};

#[cfg(not(target_os = "windows"))]
use rodio::{Decoder, OutputStream, Sink, Source};
#[cfg(not(target_os = "windows"))]
use std::io::Cursor;

#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HMODULE;
#[cfg(target_os = "windows")]
use windows::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC, SND_SYSTEM};

use crate::settings::SettingsStore;

#[cfg(target_os = "windows")]
const COPY_SOUND_BYTES: &[u8] = include_bytes!("../../assets/sounds/copy.wav");
#[cfg(not(target_os = "windows"))]
const COPY_SOUND_BYTES: &[u8] = include_bytes!("../../assets/sounds/copy.mp3");

/// 若设置启用了 `feedback.copy_sound`，异步播放一次提示音。
/// 失败仅 warn——提示音不应阻断剪贴板入库主流程。
pub fn maybe_play_copy(app: &AppHandle) {
    let enabled = app
        .try_state::<SettingsStore>()
        .map(|s| s.snapshot().clipboard.feedback.copy_sound)
        .unwrap_or(false);
    if !enabled {
        return;
    }
    spawn_play();
}

pub fn play_copy_sound_now() -> Result<(), String> {
    play_blocking()
}

fn spawn_play() {
    std::thread::Builder::new()
        .name("copy-sound".into())
        .spawn(|| {
            if let Err(err) = play_blocking() {
                log::warn!("play copy sound failed: {err}");
            }
        })
        .ok();
}

#[cfg(target_os = "windows")]
fn play_blocking() -> Result<(), String> {
    let flags = SND_MEMORY | SND_NODEFAULT | SND_SYSTEM | SND_SYNC;
    let played = unsafe {
        PlaySoundW(
            PCWSTR(COPY_SOUND_BYTES.as_ptr().cast()),
            HMODULE::default(),
            flags,
        )
    };

    if played.as_bool() {
        Ok(())
    } else {
        Err("Windows PlaySoundW returned FALSE".to_owned())
    }
}

#[cfg(not(target_os = "windows"))]
fn play_blocking() -> Result<(), String> {
    let (_stream, handle) = OutputStream::try_default().map_err(|e| e.to_string())?;
    let sink = Sink::try_new(&handle).map_err(|e| e.to_string())?;
    let source = Decoder::new(Cursor::new(COPY_SOUND_BYTES)).map_err(|e| e.to_string())?;
    sink.set_volume(1.0);
    sink.append(source.amplify(4.0));
    sink.sleep_until_end();
    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{play_blocking, COPY_SOUND_BYTES};

    #[test]
    fn bundled_copy_sound_is_a_pcm_wave_file() {
        assert_eq!(&COPY_SOUND_BYTES[0..4], b"RIFF");
        assert_eq!(&COPY_SOUND_BYTES[8..12], b"WAVE");
        assert!(COPY_SOUND_BYTES.windows(4).any(|chunk| chunk == b"fmt "));
        assert!(COPY_SOUND_BYTES.windows(4).any(|chunk| chunk == b"data"));
    }

    #[test]
    #[ignore = "plays an audible sound through the Windows default output"]
    fn windows_copy_sound_playback_smoke() {
        play_blocking().expect("PlaySoundW should accept and play the bundled WAV");
    }
}
