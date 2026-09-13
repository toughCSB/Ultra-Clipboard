use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::db::models::ClipboardSubKind;

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(https?|ftp|file)://[^\s]+$|^www\.[^\s]+\.[^\s]+$")
        .expect("invalid URL regex")
});

static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9._%+\-\u{4e00}-\u{9fa5}]+@[a-zA-Z0-9_-]+(\.[a-zA-Z0-9_-]+)+$")
        .expect("invalid email regex")
});

static HEX_COLOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^#([0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})$")
        .expect("invalid hex color regex")
});

static COLOR_FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(rgba?|hsla?|hwb|lab|lch|oklab|oklch|color|color-mix)\(.+\)$")
        .expect("invalid color fn regex")
});

static GRADIENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(repeating-)?(linear|radial|conic)-gradient\(.+\)$")
        .expect("invalid gradient regex")
});

pub fn detect_text_sub_kind(text: &str) -> Option<ClipboardSubKind> {
    let value = text.trim();
    if value.is_empty() {
        return None;
    }

    if URL_RE.is_match(value) {
        return Some(ClipboardSubKind::Url);
    }
    if EMAIL_RE.is_match(value) {
        return Some(ClipboardSubKind::Email);
    }
    if is_css_color_value(value) {
        return Some(ClipboardSubKind::Color);
    }
    if is_existing_absolute_path(value) {
        return Some(ClipboardSubKind::Path);
    }
    None
}

pub fn sanitize_css_color(text: &str) -> Option<String> {
    let value = text.trim();

    if value.is_empty() || !is_css_color_value(value) {
        return None;
    }

    Some(value.to_owned())
}

fn is_css_color_value(value: &str) -> bool {
    if HEX_COLOR_RE.is_match(value) {
        return true;
    }

    if (COLOR_FN_RE.is_match(value) || GRADIENT_RE.is_match(value)) && is_safe_css_value(value) {
        return true;
    }

    false
}

fn is_safe_css_value(value: &str) -> bool {
    if value.contains(';') || value.contains('<') || value.contains('>') {
        return false;
    }

    let lower = value.to_ascii_lowercase();
    if lower.contains("url(")
        || lower.contains("expression(")
        || lower.contains("javascript:")
        || lower.contains("@import")
    {
        return false;
    }

    let mut depth: i32 = 0;
    for ch in value.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }

    depth == 0
}

fn is_existing_absolute_path(value: &str) -> bool {
    let path = Path::new(value);
    path.is_absolute() && path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_url() {
        for s in [
            "https://example.com",
            "http://a.b/c?d=1",
            "ftp://host/file",
            "www.example.com",
            "  https://trimmed.com  ",
        ] {
            assert_eq!(
                detect_text_sub_kind(s),
                Some(ClipboardSubKind::Url),
                "should be url: {s:?}"
            );
        }

        assert_eq!(detect_text_sub_kind("example.com"), None);
        assert_eq!(detect_text_sub_kind("hello world"), None);
    }

    #[test]
    fn detects_email() {
        assert_eq!(
            detect_text_sub_kind("user@example.com"),
            Some(ClipboardSubKind::Email)
        );
        assert_eq!(
            detect_text_sub_kind("user@example.com"),
            Some(ClipboardSubKind::Email)
        );

        assert_eq!(
            detect_text_sub_kind("first.last@example.com"),
            Some(ClipboardSubKind::Email),
            "dotted local part (first.last@)"
        );
        assert_eq!(
            detect_text_sub_kind("user+tag@gmail.com"),
            Some(ClipboardSubKind::Email),
            "plus-tagged local part (user+tag@)"
        );
        assert_eq!(
            detect_text_sub_kind("name_surname@example.com"),
            Some(ClipboardSubKind::Email),
            "underscore local part (name_surname@)"
        );
        assert_eq!(
            detect_text_sub_kind("user-name@example.com"),
            Some(ClipboardSubKind::Email),
            "hyphen local part (user-name@)"
        );
        assert_eq!(
            detect_text_sub_kind("user%tag@example.com"),
            Some(ClipboardSubKind::Email),
            "percent local part (user%tag@)"
        );
        assert_eq!(detect_text_sub_kind("not@an@email"), None);
    }

    #[test]
    fn detects_color() {
        for s in [
            "#fff",
            "#FFFF",
            "#ffffff",
            "#ffffffff",
            "rgb(1,2,3)",
            "rgba(1,2,3,0.5)",
            "hsl(0, 100%, 50%)",
            "hsla(0,100%,50%,.5)",
        ] {
            assert_eq!(
                detect_text_sub_kind(s),
                Some(ClipboardSubKind::Color),
                "should be color: {s:?}"
            );
        }

        assert_eq!(detect_text_sub_kind("inherit"), None);
        assert_eq!(detect_text_sub_kind("url(#abc)"), None);
        assert_eq!(detect_text_sub_kind("#xyz"), None);
    }

    #[test]
    fn detects_modern_color_functions() {
        for s in [
            "rgb(255 87 51)",
            "rgb(255 87 51 / 0.5)",
            "hsl(0 100% 50% / 80%)",
            "hwb(120 10% 20%)",
            "lab(50% 40 30)",
            "lch(50% 40 30)",
            "oklab(0.7 0.1 0.05)",
            "oklch(0.7 0.15 30)",
            "color(display-p3 1 0 0)",
            "color(rec2020 0.5 0.2 0.8 / 0.6)",
            "color-mix(in srgb, #fff 50%, #000)",
            "color-mix(in oklch, oklch(0.7 0.15 30), red 20%)",
        ] {
            assert_eq!(
                detect_text_sub_kind(s),
                Some(ClipboardSubKind::Color),
                "should be color (modern fn): {s:?}"
            );
            assert_eq!(sanitize_css_color(s).as_deref(), Some(s));
        }
    }

    #[test]
    fn detects_gradient_as_color() {
        for s in [
            "linear-gradient(to right, #ffdde1, #ee9ca7)",
            "radial-gradient(circle, rgba(0,0,0,0.5) 0%, #fff 100%)",
            "conic-gradient(from 45deg, red, blue)",
            "repeating-linear-gradient(45deg, #000 0 10px, #fff 10px 20px)",
        ] {
            assert_eq!(
                detect_text_sub_kind(s),
                Some(ClipboardSubKind::Color),
                "should be color (gradient): {s:?}"
            );
            assert_eq!(sanitize_css_color(s).as_deref(), Some(s));
        }

        for bad in [
            "linear-gradient(to right, url(http://x))",
            "linear-gradient(to right, #fff;color:red)",
            "linear-gradient(to right, #fff",
        ] {
            assert_eq!(sanitize_css_color(bad), None, "should reject: {bad:?}");
        }
    }

    #[test]
    fn detects_existing_absolute_path_only() {
        let dir = std::env::temp_dir();
        let file = dir.join(format!(
            "ultra-clipboard-detect-{}.txt",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&file, b"x").unwrap();

        assert_eq!(
            detect_text_sub_kind(file.to_str().unwrap()),
            Some(ClipboardSubKind::Path)
        );

        assert_eq!(detect_text_sub_kind("/nope/does/not/exist/xyz"), None);
        assert_eq!(detect_text_sub_kind("src"), None);

        std::fs::remove_file(&file).ok();
    }
}
