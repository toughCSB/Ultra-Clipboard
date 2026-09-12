use crate::i18n::keys::CommandKey as Key;

pub fn label(key: Key) -> &'static str {
    match key {
        Key::DragSourceFilesMissing => "드래그한 원본 파일이 더 이상 없습니다",
        Key::DragImageMissing => "이미지 파일이 더 이상 없습니다",
        Key::DragTextEmpty => "텍스트가 비어 있습니다",
        Key::ExternalUrlUnsupported => "http 또는 https로 시작하는 링크만 열 수 있습니다",
    }
}
