use chrono::Utc;

use super::detect::detect_text_sub_kind;
use super::payload::{ClipboardPayload, TextPayload};
use super::secrets::contains_secret;
use super::storage::ImageStore;
use crate::core::Result;
use crate::db::items::content_hash;
use crate::db::models::{ClipboardItem, ClipboardKind, ClipboardSubKind, Platform};
use crate::settings::{Capture, CaptureKind, Sensitive};

pub const SUMMARY_MAX_CHARS: usize = 256;

fn make_summary(plain: &str) -> Option<String> {
    let trimmed = plain.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(SUMMARY_MAX_CHARS).collect())
}

fn current_platform() -> Platform {
    #[cfg(target_os = "macos")]
    {
        Platform::Macos
    }
    #[cfg(target_os = "windows")]
    {
        Platform::Windows
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        compile_error!("Ultra Clipboard only supports macOS and Windows")
    }
}

struct Draft {
    kind: ClipboardKind,
    sub_kind: Option<ClipboardSubKind>,
    content: String,
    search_text: Option<String>,
    summary: Option<String>,
    file_types: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    size: Option<i64>,
}

fn files_to_content(files: &[String]) -> String {
    files.join("\n")
}

fn draft_from_text(text: &TextPayload, capture: &Capture, plain_only: bool) -> Option<Draft> {
    if !capture.text && !capture.html && !capture.rtf {
        return None;
    }

    let plain = text.text.trim();
    if plain.is_empty() {
        return None;
    }

    let plain_search = Some(plain.to_owned());
    let summary = make_summary(plain);

    let html = non_empty(&text.html);
    let rtf = non_empty(&text.rtf);

    if !plain_only {
        for kind in capture.ordered_kinds() {
            if !capture.is_enabled(kind) {
                continue;
            }

            match kind {
                CaptureKind::Html => {
                    if let Some(html) = html {
                        return Some(Draft {
                            kind: ClipboardKind::Text,
                            sub_kind: Some(ClipboardSubKind::Html),
                            content: html.clone(),
                            search_text: plain_search.clone(),
                            summary: summary.clone(),
                            file_types: None,
                            width: None,
                            height: None,
                            size: Some(count_text_bytes(html)),
                        });
                    }
                }
                CaptureKind::Rtf => {
                    if let Some(rtf) = rtf {
                        return Some(Draft {
                            kind: ClipboardKind::Text,
                            sub_kind: Some(ClipboardSubKind::Rtf),
                            content: rtf.clone(),
                            search_text: plain_search.clone(),
                            summary: summary.clone(),
                            file_types: None,
                            width: None,
                            height: None,
                            size: Some(count_text_bytes(rtf)),
                        });
                    }
                }
                CaptureKind::Text => {
                    return Some(draft_plain_text(plain, plain_search, summary));
                }
                CaptureKind::Files | CaptureKind::Image => {}
            }
        }

        return None;
    }

    if !capture.text {
        return None;
    }

    Some(draft_plain_text(plain, plain_search, summary))
}

fn draft_plain_text(plain: &str, plain_search: Option<String>, summary: Option<String>) -> Draft {
    Draft {
        kind: ClipboardKind::Text,
        sub_kind: detect_text_sub_kind(plain),
        content: plain.to_owned(),
        search_text: plain_search,
        summary,
        file_types: None,
        width: None,
        height: None,
        size: Some(count_text_bytes(plain)),
    }
}

fn non_empty(value: &Option<String>) -> Option<&String> {
    value.as_ref().filter(|s| !s.trim().is_empty())
}

fn count_text_bytes(text: &str) -> i64 {
    text.len() as i64
}

fn exceeds_limit(size: usize, limit: Option<u64>) -> bool {
    limit.is_some_and(|limit| size as u64 > limit)
}

#[cfg(test)]
pub fn build_item(store: &ImageStore, payload: &ClipboardPayload) -> Result<Option<ClipboardItem>> {
    build_item_with_settings(
        store,
        payload,
        &Capture::default(),
        &Sensitive::default(),
        false,
    )
}

pub fn build_item_with_settings(
    store: &ImageStore,
    payload: &ClipboardPayload,
    capture: &Capture,
    sensitive: &Sensitive,
    plain_only: bool,
) -> Result<Option<ClipboardItem>> {
    let mut is_sensitive = false;
    let draft = match payload {
        ClipboardPayload::Text(text) => {
            if contains_secret(&text.text) {
                if !sensitive.collect_secrets {
                    return Ok(None);
                }

                is_sensitive = true;
            }

            draft_from_text(text, capture, plain_only)
        }
        ClipboardPayload::Files(files) => {
            if !capture.files {
                return Ok(None);
            }

            let content = files_to_content(files);
            if content.trim().is_empty() {
                None
            } else {
                let file_types: Vec<&str> = files
                    .iter()
                    .map(|p| {
                        if std::path::Path::new(p).is_dir() {
                            "d"
                        } else {
                            "f"
                        }
                    })
                    .collect();
                let file_types_str = file_types.join(",");

                let basenames = files
                    .iter()
                    .map(|p| {
                        std::path::Path::new(p)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(p.as_str())
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                Some(Draft {
                    kind: ClipboardKind::Files,
                    sub_kind: None,
                    content,
                    search_text: Some(basenames),
                    summary: None,
                    file_types: Some(file_types_str),
                    width: None,
                    height: None,
                    size: None,
                })
            }
        }
        ClipboardPayload::Image(image) => {
            if !capture.image {
                return Ok(None);
            }
            if exceeds_limit(image.bytes.len(), capture.max_image_bytes()) {
                log::info!(
                    "clipboard image skipped because size {} exceeds limit {:?}",
                    image.bytes.len(),
                    capture.max_image_bytes()
                );
                return Ok(None);
            }

            let stored = store.store(image)?;
            Some(Draft {
                kind: ClipboardKind::Image,
                sub_kind: None,
                content: stored.file_name,
                search_text: None,
                summary: None,
                file_types: None,
                width: Some(stored.width),
                height: Some(stored.height),
                size: Some(stored.size),
            })
        }
    };

    let Some(draft) = draft else {
        return Ok(None);
    };
    if draft.kind == ClipboardKind::Text
        && exceeds_limit(draft.content.len(), capture.max_text_bytes())
    {
        log::info!(
            "clipboard text skipped because size {} exceeds limit {:?}",
            draft.content.len(),
            capture.max_text_bytes()
        );
        return Ok(None);
    }

    let now = Utc::now();
    Ok(Some(ClipboardItem {
        id: uuid::Uuid::new_v4().to_string(),
        content_hash: content_hash(draft.kind, &draft.content),
        kind: draft.kind,
        sub_kind: draft.sub_kind,
        group_id: None,
        source_app_id: None,
        content: draft.content,
        search_text: draft.search_text,
        summary: draft.summary,
        file_types: draft.file_types,
        size: draft.size,
        width: draft.width,
        height: draft.height,
        use_count: 1,
        is_favorite: false,
        is_pinned: false,
        is_sensitive,
        platform: current_platform(),
        note: None,
        created_at: now,
        updated_at: now,
        source_app_name: None,
        source_app_icon_file: None,
        source_app_icon_path: None,
        image_thumbnail_path: None,
        file_entries: None,
        files_preview_kind: None,
        available_actions: Vec::new(),
        color_preview: None,
        display_created_at: String::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::payload::ImagePayload;

    fn text_payload(text: &str, html: Option<&str>, rtf: Option<&str>) -> ClipboardPayload {
        ClipboardPayload::Text(TextPayload {
            text: text.to_owned(),
            html: html.map(str::to_owned),
            rtf: rtf.map(str::to_owned),
        })
    }

    fn store() -> (TempDir, ImageStore) {
        let dir = TempDir::new();
        let store = ImageStore::for_test(dir.path().join("resources").join("clipboard-images"));
        (dir, store)
    }

    #[test]
    fn plain_text_runs_subtype_detection() {
        let (_d, s) = store();
        let item = build_item(&s, &text_payload("https://example.com", None, None))
            .unwrap()
            .unwrap();
        assert_eq!(item.kind, ClipboardKind::Text);
        assert_eq!(item.sub_kind, Some(ClipboardSubKind::Url));
        assert_eq!(item.content, "https://example.com");
        assert_eq!(item.search_text.as_deref(), Some("https://example.com"));
        assert_eq!(item.size, Some(19));
    }

    #[test]
    fn html_keeps_source_as_content_and_plain_as_search() {
        let (_d, s) = store();
        let item = build_item(
            &s,
            &text_payload("Hello World", Some("<b>Hello</b> World"), None),
        )
        .unwrap()
        .unwrap();
        assert_eq!(item.sub_kind, Some(ClipboardSubKind::Html));
        assert_eq!(item.content, "<b>Hello</b> World");

        assert_eq!(item.search_text.as_deref(), Some("Hello World"));
        assert_eq!(item.size, Some(18));
    }

    #[test]
    fn plain_only_stores_html_payload_as_plain_text() {
        let (_d, s) = store();
        let item = build_item_with_settings(
            &s,
            &text_payload("Hello World", Some("<b>Hello</b> World"), None),
            &Capture::default(),
            &Sensitive::default(),
            true,
        )
        .unwrap()
        .unwrap();

        assert_eq!(item.sub_kind, None);
        assert_eq!(item.content, "Hello World");
        assert_eq!(item.search_text.as_deref(), Some("Hello World"));
        assert_eq!(item.summary.as_deref(), Some("Hello World"));
    }

    #[test]
    fn html_without_os_plain_is_skipped() {
        let (_d, s) = store();
        let item = build_item(&s, &text_payload("", Some("<p>only html</p>"), None)).unwrap();
        assert!(item.is_none());
    }

    #[test]
    fn rtf_uses_os_plain_text_for_search() {
        let (_d, s) = store();
        let item = build_item(
            &s,
            &text_payload("plain repr", None, Some(r"{\rtf1 plain repr}")),
        )
        .unwrap()
        .unwrap();
        assert_eq!(item.sub_kind, Some(ClipboardSubKind::Rtf));
        assert_eq!(item.content, r"{\rtf1 plain repr}");
        assert_eq!(item.search_text.as_deref(), Some("plain repr"));
        assert_eq!(item.size, Some(18));
    }

    #[test]
    fn text_size_uses_utf8_bytes() {
        let (_d, s) = store();
        let item = build_item(&s, &text_payload("안녕", None, None))
            .unwrap()
            .unwrap();

        assert_eq!(item.size, Some(6));
    }

    #[test]
    fn text_limit_allows_equal_size_and_skips_larger_content() {
        let (_d, s) = store();
        let capture = Capture {
            max_text_mb: 1,
            ..Capture::default()
        };
        let exact = "a".repeat(1024 * 1024);
        let larger = format!("{exact}a");

        let exact_item = build_item_with_settings(
            &s,
            &text_payload(&exact, None, None),
            &capture,
            &Sensitive::default(),
            false,
        )
        .unwrap();
        let larger_item = build_item_with_settings(
            &s,
            &text_payload(&larger, None, None),
            &capture,
            &Sensitive::default(),
            false,
        )
        .unwrap();

        assert!(exact_item.is_some());
        assert!(larger_item.is_none());
    }

    #[test]
    fn text_limit_zero_is_unlimited() {
        let (_d, s) = store();
        let capture = Capture {
            max_text_mb: 0,
            ..Capture::default()
        };
        let text = "a".repeat(1024 * 1024 + 1);
        let item = build_item_with_settings(
            &s,
            &text_payload(&text, None, None),
            &capture,
            &Sensitive::default(),
            false,
        )
        .unwrap();

        assert!(item.is_some());
    }

    #[test]
    fn rich_text_limit_uses_stored_source_bytes() {
        let (_d, s) = store();
        let capture = Capture {
            max_text_mb: 1,
            ..Capture::default()
        };
        let html = format!("<p>{}</p>", "a".repeat(1024 * 1024));
        let item = build_item_with_settings(
            &s,
            &text_payload("visible", Some(&html), None),
            &capture,
            &Sensitive::default(),
            false,
        )
        .unwrap();

        assert!(item.is_none());
    }

    #[test]
    fn plain_only_limit_uses_plain_text_bytes() {
        let (_d, s) = store();
        let capture = Capture {
            max_text_mb: 1,
            ..Capture::default()
        };
        let html = format!("<p>{}</p>", "a".repeat(1024 * 1024));
        let item = build_item_with_settings(
            &s,
            &text_payload("visible", Some(&html), None),
            &capture,
            &Sensitive::default(),
            true,
        )
        .unwrap();

        assert!(item.is_some());
    }

    #[test]
    fn disabled_plain_text_yields_none() {
        let (_d, s) = store();
        let capture = Capture {
            text: false,
            ..Capture::default()
        };
        let item = build_item_with_settings(
            &s,
            &text_payload("hello", None, None),
            &capture,
            &Sensitive::default(),
            false,
        )
        .unwrap();
        assert!(item.is_none());
    }

    #[test]
    fn disabled_html_falls_back_to_plain_text_payload() {
        let (_d, s) = store();
        let capture = Capture {
            html: false,
            ..Capture::default()
        };
        let item = build_item_with_settings(
            &s,
            &text_payload("Hello World", Some("<b>Hello</b> World"), None),
            &capture,
            &Sensitive::default(),
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(item.sub_kind, None);
        assert_eq!(item.content, "Hello World");
    }

    #[test]
    fn disabled_rtf_falls_back_to_plain_text_payload() {
        let (_d, s) = store();
        let capture = Capture {
            rtf: false,
            ..Capture::default()
        };
        let item = build_item_with_settings(
            &s,
            &text_payload("plain repr", None, Some(r"{\rtf1 plain repr}")),
            &capture,
            &Sensitive::default(),
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(item.sub_kind, None);
        assert_eq!(item.content, "plain repr");
    }

    #[test]
    fn custom_capture_order_can_prefer_plain_text_over_html() {
        let (_d, s) = store();
        let capture = Capture {
            order: vec![
                CaptureKind::Text,
                CaptureKind::Html,
                CaptureKind::Rtf,
                CaptureKind::Image,
                CaptureKind::Files,
            ],
            ..Capture::default()
        };
        let item = build_item_with_settings(
            &s,
            &text_payload("Hello World", Some("<b>Hello</b> World"), None),
            &capture,
            &Sensitive::default(),
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(item.sub_kind, None);
        assert_eq!(item.content, "Hello World");
    }

    #[test]
    fn disabled_html_still_allows_enabled_rtf_payload() {
        let (_d, s) = store();
        let capture = Capture {
            html: false,
            ..Capture::default()
        };
        let item = build_item_with_settings(
            &s,
            &text_payload(
                "plain repr",
                Some("<b>plain repr</b>"),
                Some(r"{\rtf1 plain repr}"),
            ),
            &capture,
            &Sensitive::default(),
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(item.sub_kind, Some(ClipboardSubKind::Rtf));
        assert_eq!(item.content, r"{\rtf1 plain repr}");
        assert_eq!(item.search_text.as_deref(), Some("plain repr"));
    }

    #[test]
    fn collect_secrets_marks_token_text_sensitive() {
        let (_d, s) = store();
        let item = build_item_with_settings(
            &s,
            &text_payload("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890", None, None),
            &Capture::default(),
            &Sensitive {
                collect_secrets: true,
                redact_secrets: false,
            },
            false,
        )
        .unwrap()
        .unwrap();

        assert_eq!(item.kind, ClipboardKind::Text);
        assert_eq!(item.content, "sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890");
        assert!(item.is_sensitive);
    }

    #[test]
    fn collect_secrets_disabled_skips_token_text() {
        let (_d, s) = store();
        let item = build_item_with_settings(
            &s,
            &text_payload("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890", None, None),
            &Capture::default(),
            &Sensitive {
                collect_secrets: false,
                redact_secrets: true,
            },
            false,
        )
        .unwrap();

        assert!(item.is_none());
    }

    #[test]
    fn redact_secrets_marks_token_text_sensitive() {
        let (_d, s) = store();
        let item = build_item_with_settings(
            &s,
            &text_payload("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890", None, None),
            &Capture::default(),
            &Sensitive {
                collect_secrets: true,
                redact_secrets: true,
            },
            false,
        )
        .unwrap()
        .unwrap();

        assert_eq!(item.kind, ClipboardKind::Text);
        assert_eq!(item.content, "sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890");
        assert!(item.is_sensitive);
    }

    #[test]
    fn default_sensitive_settings_collect_and_mark_token_text() {
        let (_d, s) = store();
        let item = build_item_with_settings(
            &s,
            &text_payload("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890", None, None),
            &Capture::default(),
            &Sensitive::default(),
            false,
        )
        .unwrap()
        .unwrap();

        assert!(item.is_sensitive);
    }

    #[test]
    fn sensitive_settings_keep_plain_text_unmarked() {
        let (_d, s) = store();
        let item = build_item_with_settings(
            &s,
            &text_payload("hello clipboard", None, None),
            &Capture::default(),
            &Sensitive {
                collect_secrets: true,
                redact_secrets: true,
            },
            false,
        )
        .unwrap()
        .unwrap();

        assert_eq!(item.content, "hello clipboard");
        assert!(!item.is_sensitive);
    }

    #[test]
    fn files_join_with_newline() {
        let (_d, s) = store();
        let payload = ClipboardPayload::Files(vec!["/a/b.txt".to_owned(), "/c/d".to_owned()]);
        let item = build_item(&s, &payload).unwrap().unwrap();
        assert_eq!(item.kind, ClipboardKind::Files);
        assert_eq!(item.content, "/a/b.txt\n/c/d");
        assert_eq!(
            item.content_hash,
            content_hash(ClipboardKind::Files, "/a/b.txt\n/c/d")
        );
    }

    #[test]
    fn files_are_not_filtered_by_text_limit() {
        let (_d, s) = store();
        let capture = Capture {
            max_text_mb: 1,
            ..Capture::default()
        };
        let payload = ClipboardPayload::Files(vec!["/a/b.txt".to_owned()]);
        let item =
            build_item_with_settings(&s, &payload, &capture, &Sensitive::default(), false).unwrap();

        assert!(item.is_some());
    }

    #[test]
    fn image_is_stored_and_recorded() {
        let (_d, s) = store();
        let payload = ClipboardPayload::Image(ImagePayload {
            bytes: sample_png(20, 10),
            width: 20,
            height: 10,
        });
        let item = build_item(&s, &payload).unwrap().unwrap();
        assert_eq!(item.kind, ClipboardKind::Image);
        assert!(item.content.ends_with(".png"));
        assert_eq!(item.width, Some(20));
        assert_eq!(item.height, Some(10));
        assert!(item.size.unwrap() > 0);
        assert_eq!(
            item.content_hash,
            content_hash(ClipboardKind::Image, &item.content)
        );

        assert!(s.origin_path(&item.content).exists());
    }

    #[test]
    fn disabled_image_yields_none() {
        let (_d, s) = store();
        let capture = Capture {
            image: false,
            ..Capture::default()
        };
        let payload = ClipboardPayload::Image(ImagePayload {
            bytes: sample_png(20, 10),
            width: 20,
            height: 10,
        });
        let item =
            build_item_with_settings(&s, &payload, &capture, &Sensitive::default(), false).unwrap();
        assert!(item.is_none());
    }

    #[test]
    fn image_limit_skips_before_writing_origin() {
        let (_d, s) = store();
        let capture = Capture {
            max_image_mb: 1,
            ..Capture::default()
        };
        let bytes = vec![1; 1024 * 1024 + 1];
        let payload = ClipboardPayload::Image(ImagePayload {
            bytes: bytes.clone(),
            width: 20,
            height: 10,
        });
        let item =
            build_item_with_settings(&s, &payload, &capture, &Sensitive::default(), false).unwrap();

        assert!(item.is_none());
        assert!(!s
            .origin_path(&format!("{}.png", blake3::hash(&bytes).to_hex()))
            .exists());
    }

    #[test]
    fn disabled_files_yields_none() {
        let (_d, s) = store();
        let capture = Capture {
            files: false,
            ..Capture::default()
        };
        let payload = ClipboardPayload::Files(vec!["/a/b.txt".to_owned()]);
        let item =
            build_item_with_settings(&s, &payload, &capture, &Sensitive::default(), false).unwrap();
        assert!(item.is_none());
    }

    #[test]
    fn blank_text_yields_none() {
        let (_d, s) = store();
        assert!(build_item(&s, &text_payload("  \n\t", None, None))
            .unwrap()
            .is_none());
    }

    fn sample_png(w: u32, h: u32) -> Vec<u8> {
        use std::io::Cursor;
        let buf = image::RgbaImage::from_pixel(w, h, image::Rgba([1, 2, 3, 255]));
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(buf)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            let p = std::env::temp_dir()
                .join(format!("ultra-clipboard-ingest-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }
}
