use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipCategory {
    PlainText,
    Code,
    Url,
    Image,
}

impl ClipCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClipCategory::PlainText => "plain_text",
            ClipCategory::Code => "code",
            ClipCategory::Url => "url",
            ClipCategory::Image => "image",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "code" => ClipCategory::Code,
            "url" => ClipCategory::Url,
            "image" => ClipCategory::Image,
            _ => ClipCategory::PlainText,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: String,
    pub category: ClipCategory,
    pub content: String,
    pub preview: String,
    pub content_hash: String,
    pub language: Option<String>,
    pub image_path: Option<String>,
    pub image_data_url: Option<String>,
    pub byte_size: usize,
    pub is_pinned: bool,
    pub source_device: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub copy_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub trigger: String,
    pub title: String,
    pub body: String,
    pub category: String,
    pub enabled: bool,
    pub use_count: i64,
    pub updated_at: i64,
}

fn default_shortcut_toggle() -> String {
    "Alt+Space".to_string()
}

fn default_shortcut_clean() -> String {
    "CommandOrControl+Shift+C".to_string()
}

fn default_shortcut_snippets() -> String {
    "CommandOrControl+Shift+S".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub device_id: String,
    pub device_name: String,
    pub sync_folder: String,
    pub auto_sync_enabled: bool,
    pub auto_clean_urls: bool,
    pub auto_format_json: bool,
    pub auto_format_markdown: bool,
    pub snippet_expansion_enabled: bool,
    pub max_history_items: usize,
    pub pause_monitoring: bool,
    #[serde(default = "default_shortcut_toggle")]
    pub shortcut_toggle_window: String,
    #[serde(default = "default_shortcut_clean")]
    pub shortcut_clean_clipboard: String,
    #[serde(default = "default_shortcut_snippets")]
    pub shortcut_open_snippets: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            device_id: format!("win-{}", &uuid::Uuid::new_v4().to_string()[..8]),
            device_name: std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_else(|_| "Windows-PC".to_string()),
            sync_folder: String::new(),
            auto_sync_enabled: true,
            auto_clean_urls: true,
            auto_format_json: false,
            auto_format_markdown: false,
            snippet_expansion_enabled: true,
            max_history_items: 500,
            pause_monitoring: false,
            shortcut_toggle_window: default_shortcut_toggle(),
            shortcut_clean_clipboard: default_shortcut_clean(),
            shortcut_open_snippets: default_shortcut_snippets(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "record_type", rename_all = "snake_case")]
pub enum SyncRecord {
    Clip {
        item: ClipboardItem,
        deleted: bool,
    },
    Snippet {
        snippet: Snippet,
        deleted: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub exported_clips: usize,
    pub exported_snippets: usize,
    pub imported_clips: usize,
    pub imported_snippets: usize,
    pub sync_folder: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanTransformResult {
    pub original: String,
    pub result: String,
    pub changed: bool,
    pub applied_actions: Vec<String>,
    pub detected_category: ClipCategory,
    pub detected_language: Option<String>,
}

