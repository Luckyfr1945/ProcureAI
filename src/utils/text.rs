use sha2::{Digest, Sha256};

pub fn normalize_company_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let cleaned: String = lower
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
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
    raw.split_whitespace().collect::<Vec<&str>>().join(" ")
}
