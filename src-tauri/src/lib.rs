use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use std::borrow::Cow;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use ulticp_core::{
    cleaner::transform_text_pipeline,
    db::Database,
    models::{AppSettings, CleanTransformResult, ClipboardItem, Snippet, SyncReport},
    snippets::{expand_all_snippets_in_text, render_snippet_body},
    sync::synchronize_with_folder,
};

pub struct AppState {
    pub db: Arc<Database>,
    pub last_clipboard_hash: Arc<Mutex<String>>,
}

fn encode_rgba_to_png(width: usize, height: usize, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut png_buf = Vec::new();
    let encoder = PngEncoder::new(&mut png_buf);
    encoder
        .write_image(
            bytes,
            width as u32,
            height as u32,
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("Error codificando PNG: {e}"))?;
    Ok(png_buf)
}

fn write_text_to_os_clipboard(text: &str, last_hash: &Mutex<String>) -> Result<(), String> {
    let hash = Database::compute_hash(text.trim().as_bytes());
    if let Ok(mut guard) = last_hash.lock() {
        *guard = hash;
    }
    let mut cb = Clipboard::new().map_err(|e| format!("No se pudo abrir el portapapeles: {e}"))?;
    cb.set_text(text.to_string())
        .map_err(|e| format!("Error al copiar al portapapeles: {e}"))?;
    Ok(())
}

fn show_and_focus_main_window(app: &AppHandle, action_event: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit("global-shortcut-triggered", action_event);
    }
}

fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(false);
        let is_focused = window.is_focused().unwrap_or(false);
        if is_visible && is_focused {
            let _ = window.hide();
        } else {
            show_and_focus_main_window(app, "focus-search");
        }
    }
}

fn clean_current_os_clipboard(app: &AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let mut cb = Clipboard::new().map_err(|e| e.to_string())?;
    let current_text = cb
        .get_text()
        .map_err(|_| "El portapapeles no contiene texto".to_string())?;
    let trimmed = current_text.trim();
    if trimmed.is_empty() {
        return Err("El portapapeles está vacío".into());
    }

    let mut working = trimmed.to_string();
    if let Ok(snippets) = state.db.list_snippets() {
        let (expanded, triggers) = expand_all_snippets_in_text(&working, &snippets, None);
        if !triggers.is_empty() {
            working = expanded;
        }
    }

    let cleaned = transform_text_pipeline(&working, true, true, false, true);
    write_text_to_os_clipboard(&cleaned.result, &state.last_clipboard_hash)?;

    let settings = state.db.get_settings().unwrap_or_default();
    if let Ok(item) = state
        .db
        .insert_or_bump_text(&cleaned.result, &settings.device_name, false)
    {
        let _ = app.emit("clipboard-updated", &item);
    }
    let _ = app.emit("global-shortcut-triggered", "clipboard-cleaned");
    Ok(cleaned.result)
}

pub fn register_configured_shortcuts(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let toggle_str = settings.shortcut_toggle_window.trim();
    let clean_str = settings.shortcut_clean_clipboard.trim();
    let snippets_str = settings.shortcut_open_snippets.trim();

    let toggle_sc = if !toggle_str.is_empty() {
        Shortcut::from_str(toggle_str).ok()
    } else {
        None
    };

    let clean_sc = if !clean_str.is_empty() {
        Shortcut::from_str(clean_str).ok()
    } else {
        None
    };

    let snippets_sc = if !snippets_str.is_empty() {
        Shortcut::from_str(snippets_str).ok()
    } else {
        None
    };

    if let Some(sc) = toggle_sc {
        let _ = gs.on_shortcut(sc, move |app_handle, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_main_window(app_handle);
            }
        });
    }

    if let Some(sc) = clean_sc {
        let _ = gs.on_shortcut(sc, move |app_handle, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = clean_current_os_clipboard(app_handle);
            }
        });
    }

    if let Some(sc) = snippets_sc {
        let _ = gs.on_shortcut(sc, move |app_handle, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                show_and_focus_main_window(app_handle, "open-snippets");
            }
        });
    }

    Ok(())
}

fn setup_system_tray(app: &AppHandle) -> Result<(), String> {
    let toggle_item = MenuItem::with_id(
        app,
        "toggle",
        "Mostrar / Ocultar UltiCP (Alt+Space)",
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let clean_item = MenuItem::with_id(
        app,
        "clean_clip",
        "Limpiar Portapapeles Actual (Ctrl+Shift+C)",
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let snippets_item = MenuItem::with_id(
        app,
        "open_snippets",
        "Abrir Snippets (Ctrl+Shift+S)",
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let sync_item = MenuItem::with_id(
        app,
        "sync_now",
        "Sincronizar con Syncthing ahora",
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let sep = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let quit_item =
        MenuItem::with_id(app, "quit", "Salir de UltiCP", true, None::<&str>)
            .map_err(|e| e.to_string())?;

    let menu = Menu::with_items(
        app,
        &[
            &toggle_item,
            &clean_item,
            &snippets_item,
            &sync_item,
            &sep,
            &quit_item,
        ],
    )
    .map_err(|e| e.to_string())?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("UltiCP — Portapapeles Avanzado & Snippets")
        .on_menu_event(|app_handle, event| match event.id.as_ref() {
            "toggle" => toggle_main_window(app_handle),
            "clean_clip" => {
                let _ = clean_current_os_clipboard(app_handle);
            }
            "open_snippets" => show_and_focus_main_window(app_handle, "open-snippets"),
            "sync_now" => {
                let state = app_handle.state::<AppState>();
                let _ = synchronize_with_folder(&state.db);
                let _ = app_handle.emit("global-shortcut-triggered", "synced");
            }
            "quit" => app_handle.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn hide_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_clipboard_history(
    state: State<'_, AppState>,
    category: Option<String>,
    search: Option<String>,
    pinned_only: bool,
) -> Result<Vec<ClipboardItem>, String> {
    let settings = state.db.get_settings().unwrap_or_default();
    state.db.list_clipboard_items(
        category.as_deref(),
        search.as_deref(),
        pinned_only,
        settings.max_history_items,
    )
}

#[tauri::command]
fn add_manual_clip(
    state: State<'_, AppState>,
    content: String,
    pinned: bool,
    copy_to_os: bool,
) -> Result<ClipboardItem, String> {
    let settings = state.db.get_settings().unwrap_or_default();
    let item = state
        .db
        .insert_or_bump_text(&content, &settings.device_name, pinned)?;
    if copy_to_os {
        let _ = write_text_to_os_clipboard(&item.content, &state.last_clipboard_hash);
    }
    if settings.auto_sync_enabled {
        let _ = synchronize_with_folder(&state.db);
    }
    Ok(item)
}

#[tauri::command]
fn copy_to_os_clipboard(
    state: State<'_, AppState>,
    id: String,
    clean_before_copy: bool,
) -> Result<ClipboardItem, String> {
    let items = state.db.list_clipboard_items(None, None, false, 1000)?;
    let item = items
        .into_iter()
        .find(|i| i.id == id)
        .ok_or_else(|| "Elemento no encontrado".to_string())?;

    if item.category == ulticp_core::models::ClipCategory::Image {
        if let Some(ref img_path) = item.image_path {
            let dyn_img = image::open(img_path)
                .map_err(|e| format!("No se pudo abrir la imagen guardada: {e}"))?
                .into_rgba8();
            let (w, h) = dyn_img.dimensions();
            let raw_bytes = dyn_img.into_raw();
            if let Ok(mut guard) = state.last_clipboard_hash.lock() {
                *guard = item.content_hash.clone();
            }
            let mut cb = Clipboard::new().map_err(|e| e.to_string())?;
            cb.set_image(ImageData {
                width: w as usize,
                height: h as usize,
                bytes: Cow::Owned(raw_bytes),
            })
            .map_err(|e| format!("Error copiando imagen al portapapeles: {e}"))?;
        }
        return Ok(item);
    }

    let final_text = if clean_before_copy {
        let cleaned = transform_text_pipeline(&item.content, true, true, false, true);
        cleaned.result
    } else {
        item.content.clone()
    };

    write_text_to_os_clipboard(&final_text, &state.last_clipboard_hash)?;
    let settings = state.db.get_settings().unwrap_or_default();
    let updated = state
        .db
        .insert_or_bump_text(&final_text, &settings.device_name, item.is_pinned)?;
    Ok(updated)
}

#[tauri::command]
fn apply_clean_to_item(
    state: State<'_, AppState>,
    id: String,
    clean_urls: bool,
    prettify_json: bool,
    minify_json: bool,
    clean_markdown: bool,
) -> Result<ClipboardItem, String> {
    let items = state.db.list_clipboard_items(None, None, false, 1000)?;
    let item = items
        .into_iter()
        .find(|i| i.id == id)
        .ok_or_else(|| "Elemento no encontrado".to_string())?;

    let transformed = transform_text_pipeline(
        &item.content,
        clean_urls,
        prettify_json,
        minify_json,
        clean_markdown,
    );

    let updated = state.db.update_item_content(&id, &transformed.result)?;
    let _ = write_text_to_os_clipboard(&updated.content, &state.last_clipboard_hash);
    Ok(updated)
}

#[tauri::command]
fn transform_text_cmd(
    state: State<'_, AppState>,
    text: String,
    clean_urls: bool,
    prettify_json: bool,
    minify_json: bool,
    clean_markdown: bool,
    copy_result: bool,
) -> Result<CleanTransformResult, String> {
    let res = transform_text_pipeline(&text, clean_urls, prettify_json, minify_json, clean_markdown);
    if copy_result {
        write_text_to_os_clipboard(&res.result, &state.last_clipboard_hash)?;
        let settings = state.db.get_settings().unwrap_or_default();
        let _ = state
            .db
            .insert_or_bump_text(&res.result, &settings.device_name, false);
    }
    Ok(res)
}

#[tauri::command]
fn toggle_pin_clip(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let pinned = state.db.toggle_pin(&id)?;
    let settings = state.db.get_settings().unwrap_or_default();
    if settings.auto_sync_enabled {
        let _ = synchronize_with_folder(&state.db);
    }
    Ok(pinned)
}

#[tauri::command]
fn delete_clip(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.delete_clipboard_item(&id)?;
    let settings = state.db.get_settings().unwrap_or_default();
    if settings.auto_sync_enabled {
        let _ = synchronize_with_folder(&state.db);
    }
    Ok(())
}

#[tauri::command]
fn clear_unpinned_clips(state: State<'_, AppState>) -> Result<usize, String> {
    let count = state.db.clear_unpinned_history()?;
    let settings = state.db.get_settings().unwrap_or_default();
    if settings.auto_sync_enabled {
        let _ = synchronize_with_folder(&state.db);
    }
    Ok(count)
}

#[tauri::command]
fn get_snippets(state: State<'_, AppState>) -> Result<Vec<Snippet>, String> {
    state.db.list_snippets()
}

#[tauri::command]
fn save_snippet_cmd(
    state: State<'_, AppState>,
    id: Option<String>,
    trigger: String,
    title: String,
    body: String,
    category: String,
    enabled: bool,
) -> Result<Snippet, String> {
    let saved = state
        .db
        .save_snippet(id, &trigger, &title, &body, &category, enabled)?;
    let settings = state.db.get_settings().unwrap_or_default();
    if settings.auto_sync_enabled {
        let _ = synchronize_with_folder(&state.db);
    }
    Ok(saved)
}

#[tauri::command]
fn delete_snippet_cmd(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.delete_snippet(&id)?;
    let settings = state.db.get_settings().unwrap_or_default();
    if settings.auto_sync_enabled {
        let _ = synchronize_with_folder(&state.db);
    }
    Ok(())
}

#[tauri::command]
fn expand_and_copy_snippet(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let snippets = state.db.list_snippets()?;
    let snippet = snippets
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "Snippet no encontrado".to_string())?;

    let current_clip = Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok());
    let expanded = render_snippet_body(&snippet.body, current_clip.as_deref());

    write_text_to_os_clipboard(&expanded, &state.last_clipboard_hash)?;
    let _ = state.db.increment_snippet_use(&id);

    let settings = state.db.get_settings().unwrap_or_default();
    let _ = state
        .db
        .insert_or_bump_text(&expanded, &settings.device_name, false);

    Ok(expanded)
}

#[tauri::command]
fn expand_text_preview(
    state: State<'_, AppState>,
    input: String,
    copy_to_os: bool,
) -> Result<(String, Vec<String>), String> {
    let snippets = state.db.list_snippets()?;
    let current_clip = Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok());
    let (expanded, triggers) =
        expand_all_snippets_in_text(&input, &snippets, current_clip.as_deref());

    if copy_to_os && !expanded.trim().is_empty() {
        write_text_to_os_clipboard(&expanded, &state.last_clipboard_hash)?;
    }

    Ok((expanded, triggers))
}

#[tauri::command]
fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    state.db.get_settings()
}

#[tauri::command]
fn save_app_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    if !settings.sync_folder.trim().is_empty() {
        let _ = std::fs::create_dir_all(settings.sync_folder.trim());
    }
    state.db.save_settings(&settings)?;
    let _ = register_configured_shortcuts(&app, &settings);
    Ok(settings)
}

#[tauri::command]
fn sync_now_cmd(state: State<'_, AppState>) -> Result<SyncReport, String> {
    synchronize_with_folder(&state.db)
}

#[tauri::command]
fn simulate_remote_device_sync(state: State<'_, AppState>) -> Result<SyncReport, String> {
    let settings = state.db.get_settings()?;
    let sync_root = std::path::Path::new(&settings.sync_folder);
    let devices_dir = sync_root.join("devices");
    std::fs::create_dir_all(&devices_dir).map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().timestamp_millis();
    let sample_clip = ulticp_core::models::SyncRecord::Clip {
        item: ClipboardItem {
            id: format!("android-clip-{now}"),
            category: ulticp_core::models::ClipCategory::Url,
            content: "https://syncthing.net/downloads/".to_string(),
            preview: "https://syncthing.net/downloads/".to_string(),
            content_hash: Database::compute_hash(b"https://syncthing.net/downloads/"),
            language: None,
            image_path: None,
            image_data_url: None,
            byte_size: 32,
            is_pinned: false,
            source_device: "Android-Pixel9".to_string(),
            created_at: now,
            updated_at: now,
            copy_count: 1,
        },
        deleted: false,
    };

    let sample_snippet = ulticp_core::models::SyncRecord::Snippet {
        snippet: Snippet {
            id: "android-snippet-movil".to_string(),
            trigger: ";movil".to_string(),
            title: "Respuesta rápida desde Android".to_string(),
            body: "Enviado desde mi Android sincronizado vía Syncthing P2P el {{date}} a las {{time}}.".to_string(),
            category: "Móvil".to_string(),
            enabled: true,
            use_count: 1,
            updated_at: now,
        },
        deleted: false,
    };

    let remote_file = devices_dir.join("android-pixel9.jsonl");
    let content = format!(
        "{}\n{}\n",
        serde_json::to_string(&sample_clip).unwrap(),
        serde_json::to_string(&sample_snippet).unwrap()
    );
    std::fs::write(&remote_file, content).map_err(|e| e.to_string())?;

    synchronize_with_folder(&state.db)
}

fn spawn_clipboard_watcher(app_handle: AppHandle, db: Arc<Database>, last_hash: Arc<Mutex<String>>) {
    std::thread::spawn(move || {
        if let Ok(mut cb) = Clipboard::new() {
            if let Ok(text) = cb.get_text() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    if let Ok(mut guard) = last_hash.lock() {
                        *guard = Database::compute_hash(trimmed.as_bytes());
                    }
                }
            }
        }

        loop {
            std::thread::sleep(Duration::from_millis(400));

            let settings = match db.get_settings() {
                Ok(s) => s,
                Err(_) => continue,
            };

            if settings.pause_monitoring {
                continue;
            }

            let Ok(mut cb) = Clipboard::new() else {
                continue;
            };

            // 1. Check text clipboard
            if let Ok(raw_text) = cb.get_text() {
                let trimmed = raw_text.trim();
                if !trimmed.is_empty() {
                    let raw_hash = Database::compute_hash(trimmed.as_bytes());
                    let is_new = {
                        if let Ok(guard) = last_hash.lock() {
                            *guard != raw_hash
                        } else {
                            false
                        }
                    };

                    if is_new {
                        let mut final_text = trimmed.to_string();
                        let mut modified_os_clipboard = false;

                        if settings.snippet_expansion_enabled {
                            if let Ok(snippets) = db.list_snippets() {
                                let (expanded, triggers) =
                                    expand_all_snippets_in_text(&final_text, &snippets, None);
                                if !triggers.is_empty() && expanded != final_text {
                                    final_text = expanded;
                                    modified_os_clipboard = true;
                                }
                            }
                        }

                        if settings.auto_clean_urls
                            || settings.auto_format_json
                            || settings.auto_format_markdown
                        {
                            let cleaned = transform_text_pipeline(
                                &final_text,
                                settings.auto_clean_urls,
                                settings.auto_format_json,
                                false,
                                settings.auto_format_markdown,
                            );
                            if cleaned.changed {
                                final_text = cleaned.result;
                                modified_os_clipboard = true;
                            }
                        }

                        let final_hash = Database::compute_hash(final_text.as_bytes());
                        if let Ok(mut guard) = last_hash.lock() {
                            *guard = final_hash;
                        }

                        if modified_os_clipboard {
                            let _ = cb.set_text(final_text.clone());
                        }

                        if let Ok(item) =
                            db.insert_or_bump_text(&final_text, &settings.device_name, false)
                        {
                            let _ = app_handle.emit("clipboard-updated", &item);
                            if settings.auto_sync_enabled {
                                let _ = synchronize_with_folder(&db);
                            }
                        }
                        continue;
                    }
                }
            }

            // 2. Check image clipboard
            if let Ok(img) = cb.get_image() {
                if img.width > 0 && img.height > 0 && !img.bytes.is_empty() {
                    let raw_hash = Database::compute_hash(&img.bytes);
                    let is_new = {
                        if let Ok(guard) = last_hash.lock() {
                            *guard != raw_hash
                        } else {
                            false
                        }
                    };

                    if is_new {
                        if let Ok(png_bytes) =
                            encode_rgba_to_png(img.width, img.height, &img.bytes)
                        {
                            if let Ok(mut guard) = last_hash.lock() {
                                *guard = raw_hash;
                            }
                            let b64 = BASE64.encode(&png_bytes);
                            let data_url = format!("data:image/png;base64,{b64}");
                            if let Ok(item) = db.insert_or_bump_image(
                                &png_bytes,
                                img.width,
                                img.height,
                                &data_url,
                                &settings.device_name,
                            ) {
                                let _ = app_handle.emit("clipboard-updated", &item);
                                if settings.auto_sync_enabled {
                                    let _ = synchronize_with_folder(&db);
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("UltiCP");

    let db = Arc::new(
        Database::open(&data_dir)
            .expect("No se pudo inicializar la base de datos SQLite de UltiCP"),
    );
    let last_clipboard_hash = Arc::new(Mutex::new(String::new()));

    let _ = synchronize_with_folder(&db);

    let db_for_watcher = Arc::clone(&db);
    let hash_for_watcher = Arc::clone(&last_clipboard_hash);
    let db_for_setup = Arc::clone(&db);

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState {
            db,
            last_clipboard_hash,
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            if let Ok(initial_settings) = db_for_setup.get_settings() {
                let _ = register_configured_shortcuts(&handle, &initial_settings);
            }
            let _ = setup_system_tray(&handle);
            spawn_clipboard_watcher(handle, db_for_watcher, hash_for_watcher);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Keep running in system tray when user closes the window
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            hide_main_window,
            get_clipboard_history,
            add_manual_clip,
            copy_to_os_clipboard,
            apply_clean_to_item,
            transform_text_cmd,
            toggle_pin_clip,
            delete_clip,
            clear_unpinned_clips,
            get_snippets,
            save_snippet_cmd,
            delete_snippet_cmd,
            expand_and_copy_snippet,
            expand_text_preview,
            get_app_settings,
            save_app_settings,
            sync_now_cmd,
            simulate_remote_device_sync,
        ])
        .run(tauri::generate_context!())
        .expect("Error ejecutando la aplicación Tauri UltiCP");
}
