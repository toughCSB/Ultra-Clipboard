use crate::i18n::keys::ClipboardMenuKey as Key;

pub fn label(key: Key) -> &'static str {
    match key {
        Key::Paste => "붙여넣기",
        Key::PasteAsPlainText => "일반 텍스트로 붙여넣기",
        Key::PasteAsPath => "경로로 붙여넣기",
        Key::Copy => "복사",
        Key::SaveImage => "이미지 저장",
        Key::OpenLink => "링크 열기",
        Key::SendEmail => "메일 보내기",
        Key::RevealInFinder => "Finder에서 보기",
        Key::RevealInExplorer => "탐색기에서 보기",
        Key::Favorite => "즐겨찾기",
        Key::Unfavorite => "즐겨찾기 해제",
        Key::PinItem => "맨 위에 고정",
        Key::UnpinItem => "고정 해제",
        Key::MoveToGroup => "그룹으로 이동",
        Key::AddNote => "메모 추가",
        Key::EditNote => "메모 편집",
        Key::Delete => "삭제",
    }
}
