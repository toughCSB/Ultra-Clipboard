use regex::Regex;
use std::sync::OnceLock;

pub fn contains_secret(text: &str) -> bool {
    let value = text.trim();
    if value.is_empty() {
        return false;
    }

    has_private_key_block(value)
        || has_known_prefixed_token(value)
        || has_aws_access_key(value)
        || has_jwt(value)
        || has_labeled_secret(value)
}

fn has_private_key_block(text: &str) -> bool {
    static PRIVATE_KEY_RE: OnceLock<Regex> = OnceLock::new();
    PRIVATE_KEY_RE
        .get_or_init(|| {
            Regex::new(r"-----BEGIN (?:[A-Z0-9 ]+ )?PRIVATE KEY-----").expect("valid regex")
        })
        .is_match(text)
}

fn has_known_prefixed_token(text: &str) -> bool {
    static PREFIXED_TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    PREFIXED_TOKEN_RE
        .get_or_init(|| {
            Regex::new(
                r"(?x)
                (?i:
                    \bgh[pousr]_[A-Za-z0-9_]{36,}\b
                  | \bgithub_pat_[A-Za-z0-9_]{40,}\b
                  | \bsk-[A-Za-z0-9_-]{32,}\b
                  | \bsk-proj-[A-Za-z0-9_-]{32,}\b
                  | \bxox[baprs]-[A-Za-z0-9-]{20,}\b
                  | \b(?:api|access|refresh|secret)[_-]?token_[A-Za-z0-9_-]{24,}\b
                )
                ",
            )
            .expect("valid regex")
        })
        .is_match(text)
}

fn has_aws_access_key(text: &str) -> bool {
    static AWS_KEY_RE: OnceLock<Regex> = OnceLock::new();
    AWS_KEY_RE
        .get_or_init(|| Regex::new(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b").expect("valid regex"))
        .is_match(text)
}

fn has_jwt(text: &str) -> bool {
    static JWT_RE: OnceLock<Regex> = OnceLock::new();
    JWT_RE
        .get_or_init(|| {
            Regex::new(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b")
                .expect("valid regex")
        })
        .is_match(text)
}

fn has_labeled_secret(text: &str) -> bool {
    static LABELED_SECRET_RE: OnceLock<Regex> = OnceLock::new();
    LABELED_SECRET_RE
        .get_or_init(|| {
            Regex::new(
                r#"(?ix)
                \b
                (?:api[_-]?key|secret[_-]?key|client[_-]?secret|access[_-]?token|refresh[_-]?token|auth[_-]?token|bearer)
                \b
                ['"]?\s*[:=]\s*
                ['"]?
                [A-Za-z0-9][A-Za-z0-9._~+/=-]{23,}
                ['"]?
                "#,
            )
            .expect("valid regex")
        })
        .is_match(text)
}

#[cfg(test)]
mod tests {
    use super::contains_secret;

    #[test]
    fn detects_known_prefixed_tokens() {
        let github_token = ["ghp", "_abcdefghijklmnopqrstuvwxyzABCDE1234567890"].concat();
        let openai_token = ["sk-proj", "-abcdefghijklmnopqrstuvwxyzABCDE1234567890"].concat();
        let slack_token = ["xoxb", "-123456789012-abcdefABCDEFabcdefABCDEF"].concat();

        assert!(contains_secret(&github_token));
        assert!(contains_secret(&openai_token));
        assert!(contains_secret(&slack_token));
    }

    #[test]
    fn detects_private_keys_aws_keys_and_jwt() {
        let aws_key = ["AKIA", "IOSFODNN7EXAMPLE"].concat();
        let private_key = [
            "-----BEGIN ",
            "OPENSSH PRIVATE KEY",
            "-----\nabc\n-----END OPENSSH PRIVATE KEY-----",
        ]
        .concat();
        let jwt = [
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
            ".eyJzdWIiOiIxMjM0NTY3ODkwIn0",
            ".SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
        ]
        .concat();

        assert!(contains_secret(&private_key));
        assert!(contains_secret(&aws_key));
        assert!(contains_secret(&jwt));
    }

    #[test]
    fn detects_labeled_secrets() {
        let labeled_secret = ["client", "_secret = abcdefghijklmnopqrstuvwxyz123456"].concat();
        let bearer_token = ["Authorization bearer", ": abcdefghijklmnopqrstuvwxyz123456"].concat();

        assert!(contains_secret(&labeled_secret));
        assert!(contains_secret(&bearer_token));
    }

    #[test]
    fn detects_labeled_secrets_with_quoted_keys() {
        let json_double = r#"{"api_key": "dummy_secret_value_abcdefghijklmnopqr"}"#;
        assert!(contains_secret(json_double));

        let single_quoted_key = r#"'secret_key': "abcdefghijklmnopqrstuvwxyz12345678""#;
        assert!(contains_secret(single_quoted_key));

        let single_quoted_both = r#"'access_token': 'abcdefghijklmnopqrstuvwxyz123456'"#;
        assert!(contains_secret(single_quoted_both));
    }

    #[test]
    fn ignores_ordinary_text_and_short_codes() {
        assert!(!contains_secret(
            "token이라는 단어가 포함된 일반 클립보드 텍스트입니다."
        ));
        assert!(!contains_secret("인증번호 123456"));
        assert!(!contains_secret("https://example.com/path/to/resource"));
        assert!(!contains_secret(
            "AKIA is just a word without enough characters"
        ));
    }
}
