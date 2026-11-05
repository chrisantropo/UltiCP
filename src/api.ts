import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AppSettings,
  CleanTransformResult,
  ClipboardItem,
  Snippet,
  SyncReport,
} from "./types";

const isTauri = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function getClipboardHistory(
  category: string,
  search: string,
  pinnedOnly: boolean
): Promise<ClipboardItem[]> {
  if (!isTauri()) return [];
  return invoke("get_clipboard_history", {
    category: category === "all" ? null : category,
    search: search.trim() ? search.trim() : null,
    pinnedOnly,
  });
}

export async function addManualClip(
  content: string,
  pinned: boolean,
  copyToOs: boolean
): Promise<ClipboardItem> {
  return invoke("add_manual_clip", { content, pinned, copyToOs });
}

export async function copyToOsClipboard(
  id: string,
  cleanBeforeCopy: boolean
): Promise<ClipboardItem> {
  return invoke("copy_to_os_clipboard", { id, cleanBeforeCopy });
}

export async function applyCleanToItem(
  id: string,
  cleanUrls: boolean,
  prettifyJson: boolean,
  minifyJson: boolean,
  cleanMarkdown: boolean
): Promise<ClipboardItem> {
  return invoke("apply_clean_to_item", {
    id,
    cleanUrls,
    prettifyJson,
    minifyJson,
    cleanMarkdown,
  });
}

export async function transformTextCmd(
  text: string,
  cleanUrls: boolean,
  prettifyJson: boolean,
  minifyJson: boolean,
  cleanMarkdown: boolean,
  copyResult: boolean
): Promise<CleanTransformResult> {
  return invoke("transform_text_cmd", {
    text,
    cleanUrls,
    prettifyJson,
    minifyJson,
    cleanMarkdown,
    copyResult,
  });
}

export async function togglePinClip(id: string): Promise<boolean> {
  return invoke("toggle_pin_clip", { id });
}

export async function deleteClip(id: string): Promise<void> {
  return invoke("delete_clip", { id });
}

export async function clearUnpinnedClips(): Promise<number> {
  return invoke("clear_unpinned_clips");
}

export async function getSnippets(): Promise<Snippet[]> {
  if (!isTauri()) return [];
  return invoke("get_snippets");
}

export async function saveSnippetCmd(params: {
  id?: string | null;
  trigger: string;
  title: string;
  body: string;
  category: string;
  enabled: boolean;
}): Promise<Snippet> {
  return invoke("save_snippet_cmd", {
    id: params.id ?? null,
    trigger: params.trigger,
    title: params.title,
    body: params.body,
    category: params.category,
    enabled: params.enabled,
  });
}

export async function deleteSnippetCmd(id: string): Promise<void> {
  return invoke("delete_snippet_cmd", { id });
}

export async function expandAndCopySnippet(id: string): Promise<string> {
  return invoke("expand_and_copy_snippet", { id });
}

export async function expandTextPreview(
  input: string,
  copyToOs: boolean
): Promise<[string, string[]]> {
  return invoke("expand_text_preview", { input, copyToOs });
}

export async function getAppSettings(): Promise<AppSettings> {
  return invoke("get_app_settings");
}

export async function saveAppSettings(
  settings: AppSettings
): Promise<AppSettings> {
  return invoke("save_app_settings", { settings });
}

export async function syncNowCmd(): Promise<SyncReport> {
  return invoke("sync_now_cmd");
}

export async function simulateRemoteDeviceSync(): Promise<SyncReport> {
  return invoke("simulate_remote_device_sync");
}

export async function hideMainWindow(): Promise<void> {
  if (!isTauri()) return;
  return invoke("hide_main_window");
}

export async function onClipboardUpdated(
  callback: (item: ClipboardItem) => void
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const unlisten = await listen<ClipboardItem>("clipboard-updated", (event) => {
    callback(event.payload);
  });
  return unlisten;
}

export async function onGlobalShortcutTriggered(
  callback: (action: string) => void
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const unlisten = await listen<string>(
    "global-shortcut-triggered",
    (event) => {
      callback(event.payload);
    }
  );
  return unlisten;
}

