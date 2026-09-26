use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SensitiveTokenType {
    StripeApiKey,
    GitHubToken,
    AwsAccessKey,
    AwsSecretKey,
    JsonWebToken,
    BearerToken,
    PrivateKey,
    PasswordAssignment,
    OpenAiApiKey,
    EmailAddress,
}

impl SensitiveTokenType {
    pub fn label(&self) -> &'static str {
        match self {
            SensitiveTokenType::StripeApiKey => "Stripe API Key",
            SensitiveTokenType::GitHubToken => "GitHub Token",
            SensitiveTokenType::AwsAccessKey => "AWS Access Key",
            SensitiveTokenType::AwsSecretKey => "AWS Secret Key",
            SensitiveTokenType::JsonWebToken => "JSON Web Token",
            SensitiveTokenType::BearerToken => "Bearer Token",
            SensitiveTokenType::PrivateKey => "Private Key",
            SensitiveTokenType::PasswordAssignment => "Password / Secret",
            SensitiveTokenType::OpenAiApiKey => "OpenAI API Key",
            SensitiveTokenType::EmailAddress => "Email Address",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DetectedTokenMatch {
    pub token_type: SensitiveTokenType,
    pub matched_text: String,
    pub masked_preview: String,
    pub char_start: usize,
    pub char_end: usize,
    pub label: String,
}

pub type SecretMatch = DetectedTokenMatch;

pub struct TokenPatternLibrary {
    stripe_regex: Regex,
    github_regex: Regex,
    aws_access_key_regex: Regex,
    aws_secret_key_regex: Regex,
    openai_regex: Regex,
    jwt_regex: Regex,
    private_key_regex: Regex,
    password_regex: Regex,
    email_regex: Regex,
    bearer_regex: Regex,
}

static PATTERNS: OnceLock<TokenPatternLibrary> = OnceLock::new();

impl TokenPatternLibrary {
    pub fn global() -> &'static Self {
        PATTERNS.get_or_init(Self::new)
    }

    pub fn new() -> Self {
        Self {
            // Stripe: (?:r|s)k_live_[0-9a-zA-Z]{24,99}
            stripe_regex: Regex::new(r"(?:r|s)k_live_[0-9a-zA-Z]{24,99}").unwrap(),

            // GitHub: gh[pousr]_[A-Za-z0-9_]{36,255} (also supporting modern github_pat_)
            github_regex: Regex::new(r"gh[pousr]_[A-Za-z0-9_]{36,255}|github_pat_[0-9a-zA-Z_]{60,}").unwrap(),

            // AWS Access Key ID: AKIA[0-9A-Z]{16} or ASIA...
            aws_access_key_regex: Regex::new(r"(?:AKIA|ASIA)[0-9A-Z]{16}").unwrap(),

            // AWS Secret Key: 40-character base64 assignment
            aws_secret_key_regex: Regex::new(r#"(?i)(?:aws_secret_access_key|aws_secret_key)\s*[:=]\s*["']?([A-Za-z0-9/+=]{40})["']?"#).unwrap(),

            // OpenAI and generic sk-... secret keys
            openai_regex: Regex::new(r"sk-[a-zA-Z0-9_-]{32,}").unwrap(),

            // JSON Web Tokens: eyJ[A-Za-z0-9-_]+\.eyJ[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+
            jwt_regex: Regex::new(r"eyJ[A-Za-z0-9-_]+\.eyJ[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+").unwrap(),

            // Private Keys: -----BEGIN.*PRIVATE KEY-----
            private_key_regex: Regex::new(r"-----BEGIN[ A-Z0-9_-]*PRIVATE KEY-----").unwrap(),

            // Password and env variable secret assignments
            password_regex: Regex::new(r#"(?i)(?:password|passwd|secret|api_key|token)\s*[:=]\s*["']?([^\s"']{6,})["']?"#).unwrap(),

            // Email addresses
            email_regex: Regex::new(r"[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+").unwrap(),

            // HTTP Bearer tokens
            bearer_regex: Regex::new(r"(?i)bearer\s+([a-zA-Z0-9_.-]{20,})").unwrap(),
        }
    }

    /// Scan arbitrary text for sensitive credentials and return detected matches
    pub fn scan_text(&self, text: &str) -> Vec<DetectedTokenMatch> {
        let mut matches = Vec::new();

        let mut check_rule = |re: &Regex, token_type: SensitiveTokenType| {
            for mat in re.find_iter(text) {
                let s = mat.as_str();
                matches.push(DetectedTokenMatch {
                    token_type,
                    matched_text: s.to_string(),
                    masked_preview: mask_string(s),
                    char_start: mat.start(),
                    char_end: mat.end(),
                    label: token_type.label().to_string(),
                });
            }
        };

        check_rule(&self.stripe_regex, SensitiveTokenType::StripeApiKey);
        check_rule(&self.github_regex, SensitiveTokenType::GitHubToken);
        check_rule(&self.aws_access_key_regex, SensitiveTokenType::AwsAccessKey);
        check_rule(&self.jwt_regex, SensitiveTokenType::JsonWebToken);
        check_rule(&self.private_key_regex, SensitiveTokenType::PrivateKey);
        check_rule(&self.openai_regex, SensitiveTokenType::OpenAiApiKey);
        check_rule(&self.email_regex, SensitiveTokenType::EmailAddress);

        // AWS Secret Key assignments
        for caps in self.aws_secret_key_regex.captures_iter(text) {
            if let Some(sec) = caps.get(1) {
                let s = sec.as_str();
                matches.push(DetectedTokenMatch {
                    token_type: SensitiveTokenType::AwsSecretKey,
                    matched_text: s.to_string(),
                    masked_preview: mask_string(s),
                    char_start: sec.start(),
                    char_end: sec.end(),
                    label: SensitiveTokenType::AwsSecretKey.label().to_string(),
                });
            }
        }

        // Bearer tokens
        for caps in self.bearer_regex.captures_iter(text) {
            if let Some(tok) = caps.get(1) {
                let s = tok.as_str();
                matches.push(DetectedTokenMatch {
                    token_type: SensitiveTokenType::BearerToken,
                    matched_text: s.to_string(),
                    masked_preview: mask_string(s),
                    char_start: tok.start(),
                    char_end: tok.end(),
                    label: SensitiveTokenType::BearerToken.label().to_string(),
                });
            }
        }

        // Password assignments: extract the captured secret group
        for caps in self.password_regex.captures_iter(text) {
            if let Some(secret_group) = caps.get(1) {
                let s = secret_group.as_str();
                // Avoid duplicating if already matched by AWS/Bearer
                let already_matched = matches.iter().any(|m| m.char_start == secret_group.start());
                if !already_matched {
                    matches.push(DetectedTokenMatch {
                        token_type: SensitiveTokenType::PasswordAssignment,
                        matched_text: s.to_string(),
                        masked_preview: mask_string(s),
                        char_start: secret_group.start(),
                        char_end: secret_group.end(),
                        label: SensitiveTokenType::PasswordAssignment.label().to_string(),
                    });
                }
            }
        }

        // Sort matches by start position
        matches.sort_by_key(|m| m.char_start);
        matches
    }
}

/// Expose scanning function finding sensitive spans in text
pub fn find_sensitive_spans(text: &str) -> Vec<SecretMatch> {
    TokenPatternLibrary::global().scan_text(text)
}

fn mask_string(s: &str) -> String {
    let len = s.len();
    if len <= 8 {
        return "*".repeat(len);
    }
    let prefix = &s[..4];
    let suffix = &s[len - 4..];
    format!("{}...{}", prefix, suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stripe_and_github_pattern_detection() {
        let library = TokenPatternLibrary::new();

        let sample = "Here is my key sk_live_51M0abcdefghijklmnopqrstuvwxyz and token ghp_111122223333444455556666777788889999.";
        let results = library.scan_text(sample);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].token_type, SensitiveTokenType::StripeApiKey);
        assert_eq!(results[1].token_type, SensitiveTokenType::GitHubToken);

        // Also test rk_live_ restricted key and gho_ oauth token
        let sample2 = "rk_live_ab12cd34ef56gh78ij90kl12mn34 and gho_abcdefghijklmnopqrstuvwxyz1234567890";
        let results2 = find_sensitive_spans(sample2);
        assert_eq!(results2.len(), 2);
        assert_eq!(results2[0].token_type, SensitiveTokenType::StripeApiKey);
        assert_eq!(results2[1].token_type, SensitiveTokenType::GitHubToken);
    }

    #[test]
    fn test_aws_and_jwt_pattern_detection() {
        let library = TokenPatternLibrary::new();

        let sample = "AWS_KEY=AKIAIOSFODNN7EXAMPLE and JWT eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozGz64flPqE1bkmSnwK7Wu";
        let results = library.scan_text(sample);

        assert!(results.iter().any(|r| r.token_type == SensitiveTokenType::AwsAccessKey));
        assert!(results.iter().any(|r| r.token_type == SensitiveTokenType::JsonWebToken));
    }

    #[test]
    fn test_private_key_and_aws_secret_detection() {
        let sample = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA0\n-----END RSA PRIVATE KEY-----\naws_secret_access_key = 'wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY'";
        let results = find_sensitive_spans(sample);

        assert!(results.iter().any(|r| r.token_type == SensitiveTokenType::PrivateKey));
        assert!(results.iter().any(|r| r.token_type == SensitiveTokenType::AwsSecretKey));
    }

    #[test]
    fn test_email_and_password_detection() {
        let library = TokenPatternLibrary::new();

        let sample = "User email founder@panglide.dev with password = 'SuperSecretPassword2026!' in env.";
        let results = library.scan_text(sample);

        assert!(results.iter().any(|r| r.token_type == SensitiveTokenType::EmailAddress));
        assert!(results.iter().any(|r| r.token_type == SensitiveTokenType::PasswordAssignment));
    }

    #[test]
    fn test_non_sensitive_text_is_ignored() {
        let text = "Welcome to the PanGlide demonstration! Today we are showcasing the fluid 60 FPS hardware capture pipeline without any credential leaks.";
        let results = find_sensitive_spans(text);
        assert!(results.is_empty(), "Standard product demo narration must produce 0 matches");
    }
}
