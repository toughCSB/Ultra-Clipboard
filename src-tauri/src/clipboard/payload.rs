#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardPayload {
    Text(TextPayload),
    Image(ImagePayload),

    Files(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPayload {
    pub text: String,

    pub html: Option<String>,

    pub rtf: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePayload {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
