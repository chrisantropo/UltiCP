use crate::db::Database;
use crate::models::{SyncRecord, SyncReport};
use chrono::Utc;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

/// Synchronizes the local SQLite database with the configured Syncthing folder.
///
/// Architecture:
/// - `<sync_folder>/devices/<device_id>.jsonl`: Each device only writes to its own JSONL snapshot/log file.
/// - `<sync_folder>/blobs/<sha256>.png`: Content-addressed image blobs.
/// - Zero SQLite lock conflicts or `.sync-conflict` files when Windows and Android sync simultaneously via Syncthing.
pub fn synchronize_with_folder(db: &Database) -> Result<SyncReport, String> {
    let settings = db.get_settings()?;
    let sync_folder_str = settings.sync_folder.trim();
    if sync_folder_str.is_empty() {
        return Err("No se ha configurado una carpeta de sincronización (Syncthing)".into());
    }

    let sync_root = Path::new(sync_folder_str);
    let devices_dir = sync_root.join("devices");
    let blobs_dir = sync_root.join("blobs");

    fs::create_dir_all(&devices_dir).map_err(|e| format!("Error creando carpeta devices: {e}"))?;
    fs::create_dir_all(&blobs_dir).map_err(|e| format!("Error creando carpeta blobs: {e}"))?;

    let my_file_name = format!("{}.jsonl", settings.device_id);
    let mut imported_clips = 0;
    let mut imported_snippets = 0;

    // 1. Read all remote device JSONL files (including any manually dropped .jsonl files)
    if let Ok(entries) = fs::read_dir(&devices_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !fname.ends_with(".jsonl") || fname == my_file_name {
                continue;
            }

            if let Ok(file) = File::open(&path) {
                let reader = BufReader::new(file);
                for line in reader.lines().map_while(Result::ok) {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(record) = serde_json::from_str::<SyncRecord>(trimmed) {
                        match record {
                            SyncRecord::Clip { mut item, deleted } => {
                                // Check if there is an image blob in blobs_dir
                                if item.category == crate::models::ClipCategory::Image {
                                    let blob_path =
                                        blobs_dir.join(format!("{}.png", item.content_hash));
                                    if blob_path.exists() {
                                        let local_images = db.data_dir.join("images");
                                        let _ = fs::create_dir_all(&local_images);
                                        let local_target =
                                            local_images.join(format!("{}.png", item.content_hash));
                                        if !local_target.exists() {
                                            let _ = fs::copy(&blob_path, &local_target);
                                        }
                                        item.image_path =
                                            Some(local_target.to_string_lossy().to_string());
                                    }
                                }
                                if db.merge_remote_clip(&item, deleted).unwrap_or(false) {
                                    imported_clips += 1;
                                }
                            }
                            SyncRecord::Snippet { snippet, deleted } => {
                                if db.merge_remote_snippet(&snippet, deleted).unwrap_or(false) {
                                    imported_snippets += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Export our own state to `<sync_folder>/devices/<device_id>.jsonl` atomically
    let clips = db.get_all_clips_for_sync()?;
    let snippets = db.get_all_snippets_for_sync()?;

    let tmp_path = devices_dir.join(format!("{}.jsonl.tmp", settings.device_id));
    let final_path = devices_dir.join(&my_file_name);

    {
        let file = File::create(&tmp_path).map_err(|e| e.to_string())?;
        let mut writer = BufWriter::new(file);

        for (item, deleted) in &clips {
            if let Some(ref img_path) = item.image_path {
                let src = Path::new(img_path);
                if src.exists() {
                    let dst = blobs_dir.join(format!("{}.png", item.content_hash));
                    if !dst.exists() {
                        let _ = fs::copy(src, dst);
                    }
                }
            }
            let rec = SyncRecord::Clip {
                item: item.clone(),
                deleted: *deleted,
            };
            if let Ok(line) = serde_json::to_string(&rec) {
                let _ = writeln!(writer, "{line}");
            }
        }

        for (snippet, deleted) in &snippets {
            let rec = SyncRecord::Snippet {
                snippet: snippet.clone(),
                deleted: *deleted,
            };
            if let Ok(line) = serde_json::to_string(&rec) {
                let _ = writeln!(writer, "{line}");
            }
        }

        writer.flush().map_err(|e| e.to_string())?;
    }

    fs::rename(&tmp_path, &final_path).map_err(|e| e.to_string())?;

    Ok(SyncReport {
        exported_clips: clips.len(),
        exported_snippets: snippets.len(),
        imported_clips,
        imported_snippets,
        sync_folder: sync_folder_str.to_string(),
        timestamp: Utc::now().timestamp_millis(),
    })
}

