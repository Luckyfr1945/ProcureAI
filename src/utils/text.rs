use regex::Regex;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,10}\b").unwrap()
});

static WA_URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:wa\.me/|api\.whatsapp\.com/send\?(?:[^#\s]*&)?phone=)\+?(\d{8,15})").unwrap()
});

static ID_MOBILE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:\+62|0)[\s.-]?8[1-9][0-9]{1,2}[\s.-]?[0-9]{3,4}[\s.-]?[0-9]{2,5}\b").unwrap()
});

static LABELED_PHONE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:telp?|telepon|phone|hp|wa|whatsapp|hotline|call|kontak|contact)\s*[:.]?\s*(\+?\(?\d[\d\s().-]{7,18}\d)",
    )
    .unwrap()
});

pub fn extract_whatsapp_from_href(href: &str) -> Option<String> {
    WA_URL_REGEX.captures(href).and_then(|caps| {
        caps.get(1).map(|m| {
            let digits = m.as_str();
            if digits.starts_with('0') && digits.len() >= 10 {
                format!("+62{}", &digits[1..])
            } else {
                format!("+{}", digits)
            }
        })
    })
}

pub fn extract_phones_and_whatsapp(text: &str) -> Vec<String> {
    let mut results: Vec<String> = Vec::new();
    let mut seen_digits: Vec<String> = Vec::new();

    let mut push_phone = |raw: &str| {
        let trimmed = raw.trim();
        let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() < 8 || digits.len() > 15 {
            return;
        }
        // Reject obvious non-phone sequences (repeated digits like 00000000)
        if digits.chars().all(|c| c == digits.chars().next().unwrap_or('0')) {
            return;
        }
        let norm_digits = if digits.starts_with("62") {
            digits.clone()
        } else if digits.starts_with('0') && digits.len() >= 10 {
            format!("62{}", &digits[1..])
        } else {
            digits.clone()
        };
        if !seen_digits.contains(&norm_digits) {
            seen_digits.push(norm_digits);
            results.push(trimmed.to_string());
        }
    };

    for caps in WA_URL_REGEX.captures_iter(text) {
        if let Some(m) = caps.get(1) {
            push_phone(&format!("+{}", m.as_str()));
        }
    }

    for mat in ID_MOBILE_REGEX.find_iter(text) {
        push_phone(mat.as_str());
    }

    for caps in LABELED_PHONE_REGEX.captures_iter(text) {
        if let Some(m) = caps.get(1) {
            push_phone(m.as_str());
        }
    }

    results
}

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
