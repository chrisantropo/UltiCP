export type ClipCategory = "plain_text" | "code" | "url" | "image";

export interface ClipboardItem {
  id: string;
  category: ClipCategory;
  content: string;
  preview: string;
  content_hash: string;
  language?: string | null;
  image_path?: string | null;
  image_data_url?: string | null;
  byte_size: number;
  is_pinned: boolean;
  source_device: string;
  created_at: number;
  updated_at: number;
  copy_count: number;
}

export interface Snippet {
  id: string;
  trigger: string;
  title: string;
  body: string;
  category: string;
  enabled: boolean;
  use_count: number;
  updated_at: number;
}

export interface AppSettings {
  device_id: string;
  device_name: string;
  sync_folder: string;
  auto_sync_enabled: boolean;
  auto_clean_urls: boolean;
  auto_format_json: boolean;
  auto_format_markdown: boolean;
  snippet_expansion_enabled: boolean;
  max_history_items: number;
  pause_monitoring: boolean;
  shortcut_toggle_window: string;
  shortcut_clean_clipboard: string;
  shortcut_open_snippets: string;
}

export interface SyncReport {
  exported_clips: number;
  exported_snippets: number;
  imported_clips: number;
  imported_snippets: number;
  sync_folder: string;
  timestamp: number;
}

export interface CleanTransformResult {
  original: string;
  result: string;
  changed: boolean;
  applied_actions: string[];
  detected_category: ClipCategory;
  detected_language?: string | null;
}

