use crate::models::ClipCategory;
use once_cell::sync::Lazy;
use regex::Regex;
use url::Url;

static CODE_PATTERNS: Lazy<Vec<(&'static str, Regex)>> = Lazy::new(|| {
    vec![
        (
            "rust",
            Regex::new(r"(?m)^\s*(pub\s+)?(fn|struct|enum|impl|use|mod|let\s+mut)\s+\w+").unwrap(),
        ),
        (
            "typescript",
            Regex::new(r"(?m)^\s*(import\s+.*from\s+['\x22]|export\s+(const|function|interface|type|class)|const\s+\w+\s*:\s*\w+)").unwrap(),
        ),
        (
            "javascript",
            Regex::new(r"(?m)^\s*(const|let|var|function)\s+\w+\s*(=|\()|=>\s*\{").unwrap(),
        ),
        (
            "python",
            Regex::new(r"(?m)^\s*(def\s+\w+\s*\(|class\s+\w+.*:|from\s+[\w.]+\s+import\s+|import\s+\w+)").unwrap(),
        ),
        (
            "sql",
            Regex::new(r"(?mi)^\s*(SELECT\s+.+\s+FROM|INSERT\s+INTO|UPDATE\s+\w+\s+SET|CREATE\s+TABLE|ALTER\s+TABLE)").unwrap(),
        ),
        (
            "html",
            Regex::new(r"(?is)^\s*(<!DOCTYPE\s+html>|<html|<div|<section|<script|<style|<svg|<table)").unwrap(),
        ),
        (
            "css",
            Regex::new(r"(?m)^\s*([.#]?[\w-]+\s*\{\s*[\w-]+\s*:|@media\s+|@keyframes\s+)").unwrap(),
        ),
        (
            "shell",
            Regex::new(r"(?m)^\s*(#!/bin/(ba)?sh|sudo\s+|apt(-get)?\s+|docker\s+(run|build|compose)|git\s+(clone|commit|push|pull|checkout)|cargo\s+\w+|npm\s+(install|run))").unwrap(),
        ),
    ]
});

/// Classifies text content into ClipCategory and returns an optional detected language.
pub fn classify_text(text: &str) -> (ClipCategory, Option<String>) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return (ClipCategory::PlainText, None);
    }

    // 1. Check if it is a single standalone URL
    if !trimmed.contains('\n') && !trimmed.contains(' ') {
        if let Ok(parsed) = Url::parse(trimmed) {
            if matches!(parsed.scheme(), "http" | "https" | "ftp" | "mailto" | "ws" | "wss") {
                return (ClipCategory::Url, None);
            }
        }
    }

    // 2. Check if it is valid JSON object or array
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
            return (ClipCategory::Code, Some("json".to_string()));
        }
    }

    // 3. Check language regex patterns
    for (lang, regex) in CODE_PATTERNS.iter() {
        if regex.is_match(trimmed) {
            return (ClipCategory::Code, Some((*lang).to_string()));
        }
    }

    // 4. General code heuristics (braces, semicolons, indentation)
    let lines: Vec<&str> = trimmed.lines().collect();
    if lines.len() >= 2 {
        let code_punctuation_lines = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.ends_with(';')
                    || t.ends_with('{')
                    || t.ends_with('}')
                    || t.starts_with("//")
                    || t.starts_with("#include")
            })
            .count();

        if code_punctuation_lines * 2 >= lines.len() {
            return (ClipCategory::Code, Some("code".to_string()));
        }
    }

    (ClipCategory::PlainText, None)
}

/// Builds a compact preview string for UI cards.
pub fn build_preview(text: &str, max_chars: usize) -> String {
    let collapsed: String = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ↵ ");
    if collapsed.chars().count() <= max_chars {
        collapsed
    } else {
        let mut s: String = collapsed.chars().take(max_chars).collect();
        s.push('…');
        s
    }
}

