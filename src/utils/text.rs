use regex::Regex;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,10}\b").unwrap()
});

pub fn extract_valid_emails(text: &str) -> Vec<String> {
    let mut emails = Vec::new();
    for mat in EMAIL_REGEX.find_iter(text) {
        let candidate = mat.as_str().to_lowercase();
        // Disqualify code, CSS, JS, templates, image files
        if candidate.contains("example.com")
            || candidate.contains("@type")
            || candidate.contains("@media")
            || candidate.contains("@context")
            || candidate.contains("@import")
            || candidate.contains("@keyframes")
            || candidate.contains(".png")
            || candidate.contains(".jpg")
            || candidate.contains(".jpeg")
            || candidate.contains(".gif")
            || candidate.contains(".svg")
            || candidate.contains(".webp")
            || candidate.contains(".js")
            || candidate.contains(".css")
            || candidate.contains("json")
            || candidate.contains("parse")
            || candidate.contains("border")
            || candidate.contains("color")
            || candidate.contains("width")
            || candidate.contains("height")
            || candidate.contains('\\')
            || candidate.contains('"')
            || candidate.contains('\'')
            || candidate.contains('{')
            || candidate.contains('}')
            || candidate.contains('[')
            || candidate.contains(']')
        {
            continue;
        }

        let parts: Vec<&str> = candidate.split('@').collect();
        if parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.') {
            if !emails.contains(&candidate) {
                emails.push(candidate);
            }
        }
    }
    emails
}

pub fn normalize_company_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let cleaned: String = lower
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect();

    let tokens: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|t| {
            !matches!(
                *t,
                "inc"
                    | "incorporated"
                    | "ltd"
                    | "limited"
                    | "llc"
                    | "corp"
                    | "corporation"
                    | "co"
                    | "company"
                    | "gmbh"
                    | "pt"
                    | "cv"
                    | "tbk"
                    | "sa"
                    | "bv"
                    | "srl"
                    | "group"
            )
        })
        .collect();

    tokens.join(" ")
}

pub fn calculate_content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn clean_text(raw: &str) -> String {
    raw.split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}
