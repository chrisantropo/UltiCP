use crate::models::Snippet;
use chrono::Local;
use uuid::Uuid;

/// Expands dynamic placeholders inside a snippet body.
/// Supported placeholders:
/// - `{{date}}` -> YYYY-MM-DD
/// - `{{time}}` -> HH:MM
/// - `{{datetime}}` -> YYYY-MM-DD HH:MM:SS
/// - `{{uuid}}` -> Random UUID v4
/// - `{{clipboard}}` -> Current clipboard text
pub fn render_snippet_body(body: &str, current_clipboard: Option<&str>) -> String {
    let now = Local::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    let time_str = now.format("%H:%M").to_string();
    let datetime_str = now.format("%Y-%m-%d %H:%M:%S").to_string();

    let mut out = body
        .replace("{{date}}", &date_str)
        .replace("{{time}}", &time_str)
        .replace("{{datetime}}", &datetime_str)
        .replace("{{cursor}}", "");

    while out.contains("{{uuid}}") {
        out = out.replacen("{{uuid}}", &Uuid::new_v4().to_string(), 1);
    }

    if out.contains("{{clipboard}}") {
        let clip_val = current_clipboard.unwrap_or("");
        out = out.replace("{{clipboard}}", clip_val);
    }

    out
}

/// Checks if the end of `typed_buffer` matches any enabled snippet trigger,
/// or replaces all snippet triggers found in a full text block.
pub fn find_matching_suffix_snippet<'a>(
    typed_buffer: &str,
    snippets: &'a [Snippet],
) -> Option<&'a Snippet> {
    for snippet in snippets {
        if !snippet.enabled || snippet.trigger.is_empty() {
            continue;
        }
        if typed_buffer.ends_with(&snippet.trigger) {
            return Some(snippet);
        }
    }
    None
}

/// Expands all enabled snippet triggers inside an arbitrary input text.
pub fn expand_all_snippets_in_text(
    input: &str,
    snippets: &[Snippet],
    current_clipboard: Option<&str>,
) -> (String, Vec<String>) {
    let mut result = input.to_string();
    let mut expanded_triggers = Vec::new();

    // Sort by longest trigger first to avoid prefix collisions
    let mut sorted: Vec<&Snippet> = snippets.iter().filter(|s| s.enabled && !s.trigger.is_empty()).collect();
    sorted.sort_by(|a, b| b.trigger.len().cmp(&a.trigger.len()));

    for s in sorted {
        if result.contains(&s.trigger) {
            let rendered = render_snippet_body(&s.body, current_clipboard);
            result = result.replace(&s.trigger, &rendered);
            expanded_triggers.push(s.trigger.clone());
        }
    }

    (result, expanded_triggers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snippet_expansion() {
        let snippets = vec![Snippet {
            id: "1".into(),
            trigger: ";saludo".into(),
            title: "Saludo formal".into(),
            body: "Hola, te adjunto el enlace: {{clipboard}}".into(),
            category: "General".into(),
            enabled: true,
            use_count: 0,
            updated_at: 0,
        }];

        let (expanded, triggers) = expand_all_snippets_in_text(
            "Mensaje: ;saludo",
            &snippets,
            Some("https://ulticp.dev"),
        );
        assert_eq!(triggers, vec![";saludo"]);
        assert_eq!(expanded, "Mensaje: Hola, te adjunto el enlace: https://ulticp.dev");
    }
}

