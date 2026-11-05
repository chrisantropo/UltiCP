use crate::classifier::classify_text;
use crate::models::CleanTransformResult;
use once_cell::sync::Lazy;
use regex::Regex;
use url::Url;

static URL_FINDER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"https?://[^\s<>()\x22'\]]+").unwrap());

static TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "utm_id",
    "utm_name",
    "utm_reader",
    "utm_viz_id",
    "utm_pubreferrer",
    "utm_swu",
    "fbclid",
    "gclid",
    "gclsrc",
    "dclid",
    "msclkid",
    "twclid",
    "ttclid",
    "yclid",
    "mc_cid",
    "mc_eid",
    "igshid",
    "igsh",
    "_hsenc",
    "_hsmi",
    "hsCtaTracking",
    "mkt_tok",
    "vero_id",
    "oly_enc_id",
    "oly_anon_id",
    "rb_clickid",
    "s_kwcid",
    "spm",
    "scm",
    "ref_src",
    "ref_url",
    "feature",
];

fn is_tracking_param(host: Option<&str>, key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    if lower.starts_with("utm_") || lower.starts_with("hsa_") || lower.starts_with("matomo_") {
        return true;
    }
    if TRACKING_PARAMS.contains(&lower.as_str()) {
        return true;
    }
    // YouTube / Spotify / Amazon specific tracking params
    if let Some(h) = host {
        let h_lower = h.to_ascii_lowercase();
        if (h_lower.contains("youtube.com")
            || h_lower.contains("youtu.be")
            || h_lower.contains("spotify.com"))
            && lower == "si"
        {
            return true;
        }
        if h_lower.contains("amazon.")
            && matches!(
                lower.as_str(),
                "pd_rd_r"
                    | "pd_rd_w"
                    | "pd_rd_wg"
                    | "pf_rd_p"
                    | "pf_rd_r"
                    | "qid"
                    | "sr"
                    | "crid"
                    | "sprefix"
                    | "dib"
                    | "dib_tag"
            )
        {
            return true;
        }
    }
    false
}

/// Cleans tracking parameters from a single URL string.
pub fn clean_single_url(raw_url: &str) -> (String, usize) {
    let Ok(mut parsed) = Url::parse(raw_url) else {
        return (raw_url.to_string(), 0);
    };

    let host = parsed.host_str().map(|s| s.to_string());
    let pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if pairs.is_empty() {
        return (raw_url.to_string(), 0);
    }

    let mut kept = Vec::with_capacity(pairs.len());
    let mut removed_count = 0;

    for (k, v) in pairs {
        if is_tracking_param(host.as_deref(), &k) {
            removed_count += 1;
        } else {
            kept.push((k, v));
        }
    }

    if removed_count == 0 {
        return (raw_url.to_string(), 0);
    }

    if kept.is_empty() {
        parsed.set_query(None);
    } else {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in kept {
            serializer.append_pair(&k, &v);
        }
        let new_query = serializer.finish();
        parsed.set_query(Some(&new_query));
    }

    (parsed.to_string(), removed_count)
}

/// Cleans tracking parameters from all URLs found in the given text.
pub fn clean_tracking_urls_in_text(text: &str) -> (String, usize) {
    let mut total_removed = 0;
    let result = URL_FINDER.replace_all(text, |caps: &regex::Captures| {
        let matched = &caps[0];
        // Avoid trailing punctuation like '.', ',', ';', '!', '?'
        let trimmed = matched.trim_end_matches(&['.', ',', ';', ':', '!', '?'][..]);
        let suffix = &matched[trimmed.len()..];
        let (cleaned, count) = clean_single_url(trimmed);
        total_removed += count;
        format!("{cleaned}{suffix}")
    });
    (result.into_owned(), total_removed)
}

/// Converts minified or unformatted JSON into pretty-printed 2-space JSON.
pub fn format_json_pretty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return None;
    }
    let val: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    serde_json::to_string_pretty(&val).ok()
}

/// Converts pretty JSON into minified single-line JSON.
pub fn format_json_minify(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return None;
    }
    let val: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    serde_json::to_string(&val).ok()
}

static HEADING_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\s*#{1,6})([^\s#].*)$").unwrap());
static BULLET_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\s*)[\*\+]\s+(.*)$").unwrap());

/// Formats and normalizes Markdown text (headings, list markers, trailing spaces, excess empty lines, URLs).
pub fn format_markdown(text: &str) -> String {
    let mut out_lines: Vec<String> = Vec::new();
    let mut in_code_block = false;
    let mut empty_streak = 0;

    for raw_line in text.lines() {
        let trimmed_end = raw_line.trim_end();
        if trimmed_end.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            empty_streak = 0;
            out_lines.push(trimmed_end.to_string());
            continue;
        }

        if in_code_block {
            out_lines.push(raw_line.to_string());
            continue;
        }

        if trimmed_end.is_empty() {
            empty_streak += 1;
            if empty_streak <= 1 {
                out_lines.push(String::new());
            }
            continue;
        }
        empty_streak = 0;

        // Normalize `#Heading` -> `# Heading`
        let line = if let Some(caps) = HEADING_REGEX.captures(trimmed_end) {
            format!("{} {}", caps[1].trim_start(), caps[2].trim())
        } else if let Some(caps) = BULLET_REGEX.captures(trimmed_end) {
            // Normalize `* item` or `+ item` -> `- item`
            format!("{}- {}", &caps[1], &caps[2])
        } else {
            trimmed_end.to_string()
        };

        out_lines.push(line);
    }

    out_lines.join("\n").trim().to_string()
}

/// Runs the requested cleaning pipeline on `text` and returns a detailed CleanTransformResult.
pub fn transform_text_pipeline(
    text: &str,
    clean_urls: bool,
    prettify_json: bool,
    minify_json: bool,
    clean_markdown: bool,
) -> CleanTransformResult {
    let mut current = text.to_string();
    let mut applied_actions = Vec::new();

    if clean_urls {
        let (cleaned, removed_params) = clean_tracking_urls_in_text(&current);
        if removed_params > 0 && cleaned != current {
            current = cleaned;
            applied_actions.push(format!(
                "Eliminados {removed_params} parámetros de tracking en URL"
            ));
        }
    }

    if prettify_json {
        if let Some(pretty) = format_json_pretty(&current) {
            if pretty != current {
                current = pretty;
                applied_actions.push("JSON formateado (legible)".to_string());
            }
        }
    } else if minify_json {
        if let Some(mini) = format_json_minify(&current) {
            if mini != current {
                current = mini;
                applied_actions.push("JSON minificado".to_string());
            }
        }
    }

    if clean_markdown && !prettify_json && !minify_json {
        // Only apply markdown normalization if it's not pure JSON
        if format_json_pretty(&current).is_none() {
            let formatted_md = format_markdown(&current);
            if formatted_md != current {
                current = formatted_md;
                applied_actions.push("Markdown normalizado".to_string());
            }
        }
    }

    let changed = current != text;
    let (detected_category, detected_language) = classify_text(&current);

    CleanTransformResult {
        original: text.to_string(),
        result: current,
        changed,
        applied_actions,
        detected_category,
        detected_language,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_tracking_urls() {
        let raw = "Mira https://example.com/articulo?id=42&utm_source=twitter&utm_medium=social&fbclid=IwAR12345!";
        let (cleaned, count) = clean_tracking_urls_in_text(raw);
        assert_eq!(count, 3);
        assert_eq!(cleaned, "Mira https://example.com/articulo?id=42!");
    }

    #[test]
    fn test_format_json() {
        let minified = r#"{"name":"UltiCP","features":["clipboard","snippets","sync"],"active":true}"#;
        let pretty = format_json_pretty(minified).unwrap();
        assert!(pretty.contains("\n  \"name\": \"UltiCP\""));
    }

    #[test]
    fn test_format_markdown() {
        let dirty = "#Titulo sin espacio\n\n\n* Punto 1   \n+ Punto 2\n";
        let clean = format_markdown(dirty);
        assert_eq!(clean, "# Titulo sin espacio\n\n- Punto 1\n- Punto 2");
    }
}

