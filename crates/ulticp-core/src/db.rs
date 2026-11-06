use crate::classifier::{build_preview, classify_text};
use crate::models::{AppSettings, ClipCategory, ClipboardItem, Snippet};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

pub struct Database {
    conn: Mutex<Connection>,
    pub data_dir: PathBuf,
}

impl Database {
    pub fn open(data_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let db_path = data_dir.join("ulticp_local.sqlite3");
        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS clipboard_items (
                id TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                content TEXT NOT NULL,
                preview TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                language TEXT,
                image_path TEXT,
                image_data_url TEXT,
                byte_size INTEGER NOT NULL DEFAULT 0,
                is_pinned INTEGER NOT NULL DEFAULT 0,
                source_device TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                copy_count INTEGER NOT NULL DEFAULT 1,
                deleted INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_clips_updated_at ON clipboard_items(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_clips_category ON clipboard_items(category);
            CREATE INDEX IF NOT EXISTS idx_clips_hash ON clipboard_items(content_hash);

            CREATE TABLE IF NOT EXISTS snippets (
                id TEXT PRIMARY KEY,
                trigger_key TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                category TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                use_count INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL,
                deleted INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                json_data TEXT NOT NULL
            );
            ",
        )
        .map_err(|e| e.to_string())?;

        let db = Self {
            conn: Mutex::new(conn),
            data_dir: data_dir.to_path_buf(),
        };
        db.seed_defaults_if_empty()?;
        Ok(db)
    }

    pub fn compute_hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    fn seed_defaults_if_empty(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Ensure default settings exist
        let settings_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))
            .unwrap_or(0);
        if settings_count == 0 {
            let mut default_settings = AppSettings::default();
            let default_sync = self.data_dir.join("syncthing_shared");
            let _ = std::fs::create_dir_all(&default_sync);
            default_settings.sync_folder = default_sync.to_string_lossy().to_string();
            let json = serde_json::to_string(&default_settings).map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO settings (id, json_data) VALUES (1, ?1)",
                params![json],
            )
            .map_err(|e| e.to_string())?;
        }

        // Seed starter snippets if table is empty
        let snippet_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM snippets", [], |r| r.get(0))
            .unwrap_or(0);

        if snippet_count == 0 {
            let now = Utc::now().timestamp_millis();
            let starter_snippets = vec![
                (
                    ";firma",
                    "Firma de correo profesional",
                    "Un cordial saludo,\n\nEquipo de Ingeniería\nFecha: {{date}} ({{time}})",
                    "Trabajo",
                ),
                (
                    ";reunion",
                    "Plantilla de notas de reunión",
                    "## Notas de Reunión ({{date}})\n\n### Asistentes\n- \n\n### Acuerdos\n- \n\n### Próximos pasos\n- [ ] ",
                    "Productividad",
                ),
                (
                    ";pr",
                    "Plantilla Pull Request",
                    "### Resumen de cambios\n- \n\n### Verificación\n- [x] Compila sin warnings\n- [x] Tests unitarios en verde\n\nRef: {{clipboard}}",
                    "Código",
                ),
                (
                    ";fecha",
                    "Fecha y hora actual ISO",
                    "{{datetime}}",
                    "Utilidades",
                ),
                (
                    ";uuid",
                    "Generar identificador UUID v4",
                    "{{uuid}}",
                    "Utilidades",
                ),
            ];

            for (trigger, title, body, category) in starter_snippets {
                conn.execute(
                    "INSERT INTO snippets (id, trigger_key, title, body, category, enabled, use_count, updated_at, deleted)
                     VALUES (?1, ?2, ?3, ?4, ?5, 1, 0, ?6, 0)",
                    params![Uuid::new_v4().to_string(), trigger, title, body, category, now],
                )
                .map_err(|e| e.to_string())?;
            }
        }

        // Seed starter clipboard items if table is empty so user immediately sees all categories
        let clip_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap_or(0);

        if clip_count == 0 {
            drop(conn);
            let device = "Windows-PC";
            let _ = self.insert_or_bump_text(
                "Bienvenido a UltiCP — Tu portapapeles inteligente y gestor de snippets Local-First para Windows y Android. Copia cualquier texto, código, URL o imagen y aparecerá clasificado automáticamente.",
                device,
                true,
            );
            let _ = self.insert_or_bump_text(
                "https://github.com/tauri-apps/tauri?utm_source=newsletter&utm_medium=email&fbclid=IwAR998877",
                device,
                false,
            );
            let _ = self.insert_or_bump_text(
                "{\n  \"app\": \"UltiCP\",\n  \"platforms\": [\"Windows\", \"Android\"],\n  \"sync\": \"Syncthing P2P (JSONL + Blobs)\",\n  \"local_first\": true\n}",
                device,
                false,
            );
        }

        Ok(())
    }

    pub fn get_settings(&self) -> Result<AppSettings, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let json: String = conn
            .query_row("SELECT json_data FROM settings WHERE id = 1", [], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let json = serde_json::to_string(settings).map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO settings (id, json_data) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET json_data = excluded.json_data",
            params![json],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn insert_or_bump_text(
        &self,
        text: &str,
        source_device: &str,
        pinned: bool,
    ) -> Result<ClipboardItem, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("El contenido está vacío".into());
        }

        let content_hash = Self::compute_hash(trimmed.as_bytes());
        let (category, language) = classify_text(trimmed);
        let preview = build_preview(trimmed, 180);
        let byte_size = trimmed.len();
        let now = Utc::now().timestamp_millis();

        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Check if an item with the same content_hash already exists
        let existing_id: Option<(String, i64, bool)> = conn
            .query_row(
                "SELECT id, copy_count, is_pinned FROM clipboard_items WHERE content_hash = ?1 LIMIT 1",
                params![content_hash],
                |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0)),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        if let Some((id, count, was_pinned)) = existing_id {
            let new_count = count + 1;
            let is_pinned = was_pinned || pinned;
            conn.execute(
                "UPDATE clipboard_items
                 SET updated_at = ?1, copy_count = ?2, is_pinned = ?3, deleted = 0, category = ?4, language = ?5
                 WHERE id = ?6",
                params![
                    now,
                    new_count,
                    if is_pinned { 1 } else { 0 },
                    category.as_str(),
                    language,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;

            return Ok(ClipboardItem {
                id,
                category,
                content: trimmed.to_string(),
                preview,
                content_hash,
                language,
                image_path: None,
                image_data_url: None,
                byte_size,
                is_pinned,
                source_device: source_device.to_string(),
                created_at: now,
                updated_at: now,
                copy_count: new_count,
            });
        }

        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO clipboard_items (
                id, category, content, preview, content_hash, language,
                image_path, image_data_url, byte_size, is_pinned,
                source_device, created_at, updated_at, copy_count, deleted
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, ?7, ?8, ?9, ?10, ?10, 1, 0)",
            params![
                id,
                category.as_str(),
                trimmed,
                preview,
                content_hash,
                language,
                byte_size as i64,
                if pinned { 1 } else { 0 },
                source_device,
                now,
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(ClipboardItem {
            id,
            category,
            content: trimmed.to_string(),
            preview,
            content_hash,
            language,
            image_path: None,
            image_data_url: None,
            byte_size,
            is_pinned: pinned,
            source_device: source_device.to_string(),
            created_at: now,
            updated_at: now,
            copy_count: 1,
        })
    }

    pub fn insert_or_bump_image(
        &self,
        png_bytes: &[u8],
        width: usize,
        height: usize,
        data_url: &str,
        source_device: &str,
    ) -> Result<ClipboardItem, String> {
        let content_hash = Self::compute_hash(png_bytes);
        let now = Utc::now().timestamp_millis();
        let images_dir = self.data_dir.join("images");
        std::fs::create_dir_all(&images_dir).map_err(|e| e.to_string())?;
        let img_file = images_dir.join(format!("{content_hash}.png"));
        if !img_file.exists() {
            std::fs::write(&img_file, png_bytes).map_err(|e| e.to_string())?;
        }
        let image_path_str = img_file.to_string_lossy().to_string();
        let preview = format!("Imagen PNG ({width}×{height} px)");
        let byte_size = png_bytes.len();

        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let existing_id: Option<(String, i64, bool)> = conn
            .query_row(
                "SELECT id, copy_count, is_pinned FROM clipboard_items WHERE content_hash = ?1 LIMIT 1",
                params![content_hash],
                |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0)),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        if let Some((id, count, is_pinned)) = existing_id {
            let new_count = count + 1;
            conn.execute(
                "UPDATE clipboard_items SET updated_at = ?1, copy_count = ?2, deleted = 0 WHERE id = ?3",
                params![now, new_count, id],
            )
            .map_err(|e| e.to_string())?;

            return Ok(ClipboardItem {
                id,
                category: ClipCategory::Image,
                content: preview.clone(),
                preview,
                content_hash,
                language: None,
                image_path: Some(image_path_str),
                image_data_url: Some(data_url.to_string()),
                byte_size,
                is_pinned,
                source_device: source_device.to_string(),
                created_at: now,
                updated_at: now,
                copy_count: new_count,
            });
        }

        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO clipboard_items (
                id, category, content, preview, content_hash, language,
                image_path, image_data_url, byte_size, is_pinned,
                source_device, created_at, updated_at, copy_count, deleted
            ) VALUES (?1, 'image', ?2, ?2, ?3, NULL, ?4, ?5, ?6, 0, ?7, ?8, ?8, 1, 0)",
            params![
                id,
                preview,
                content_hash,
                image_path_str,
                data_url,
                byte_size as i64,
                source_device,
                now,
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(ClipboardItem {
            id,
            category: ClipCategory::Image,
            content: preview.clone(),
            preview,
            content_hash,
            language: None,
            image_path: Some(image_path_str),
            image_data_url: Some(data_url.to_string()),
            byte_size,
            is_pinned: false,
            source_device: source_device.to_string(),
            created_at: now,
            updated_at: now,
            copy_count: 1,
        })
    }

    pub fn list_clipboard_items(
        &self,
        category_filter: Option<&str>,
        search_query: Option<&str>,
        pinned_only: bool,
        limit: usize,
    ) -> Result<Vec<ClipboardItem>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut sql = String::from(
            "SELECT id, category, content, preview, content_hash, language,
                    image_path, image_data_url, byte_size, is_pinned,
                    source_device, created_at, updated_at, copy_count
             FROM clipboard_items
             WHERE deleted = 0",
        );

        if pinned_only {
            sql.push_str(" AND is_pinned = 1");
        }
        if let Some(cat) = category_filter {
            if !cat.is_empty() && cat != "all" {
                sql.push_str(&format!(" AND category = '{}'", cat.replace('\'', "")));
            }
        }
        sql.push_str(" ORDER BY is_pinned DESC, updated_at DESC");

        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ClipboardItem {
                    id: row.get(0)?,
                    category: ClipCategory::from_str(&row.get::<_, String>(1)?),
                    content: row.get(2)?,
                    preview: row.get(3)?,
                    content_hash: row.get(4)?,
                    language: row.get(5)?,
                    image_path: row.get(6)?,
                    image_data_url: row.get(7)?,
                    byte_size: row.get::<_, i64>(8)? as usize,
                    is_pinned: row.get::<_, i64>(9)? != 0,
                    source_device: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                    copy_count: row.get(13)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let query_lower = search_query
            .map(|q| q.trim().to_lowercase())
            .filter(|q| !q.is_empty());

        let mut items = Vec::new();
        for r in rows {
            let item = r.map_err(|e| e.to_string())?;
            if let Some(ref q) = query_lower {
                let matches = item.content.to_lowercase().contains(q)
                    || item
                        .language
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(q)
                    || item.source_device.to_lowercase().contains(q);
                if !matches {
                    continue;
                }
            }
            items.push(item);
            if items.len() >= limit {
                break;
            }
        }
        Ok(items)
    }

    pub fn toggle_pin(&self, id: &str) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let current: i64 = conn
            .query_row(
                "SELECT is_pinned FROM clipboard_items WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let next = if current == 0 { 1 } else { 0 };
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE clipboard_items SET is_pinned = ?1, updated_at = ?2 WHERE id = ?3",
            params![next, now, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(next != 0)
    }

    pub fn update_item_content(&self, id: &str, new_content: &str) -> Result<ClipboardItem, String> {
        let trimmed = new_content.trim();
        let (category, language) = classify_text(trimmed);
        let preview = build_preview(trimmed, 180);
        let content_hash = Self::compute_hash(trimmed.as_bytes());
        let byte_size = trimmed.len();
        let now = Utc::now().timestamp_millis();

        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE clipboard_items
             SET content = ?1, preview = ?2, category = ?3, language = ?4,
                 content_hash = ?5, byte_size = ?6, updated_at = ?7
             WHERE id = ?8",
            params![
                trimmed,
                preview,
                category.as_str(),
                language,
                content_hash,
                byte_size as i64,
                now,
                id
            ],
        )
        .map_err(|e| e.to_string())?;
        drop(conn);

        let all = self.list_clipboard_items(None, None, false, 1000)?;
        all.into_iter()
            .find(|i| i.id == id)
            .ok_or_else(|| "Item no encontrado".to_string())
    }

    pub fn delete_clipboard_item(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE clipboard_items SET deleted = 1, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn clear_unpinned_history(&self) -> Result<usize, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp_millis();
        let affected = conn
            .execute(
                "UPDATE clipboard_items SET deleted = 1, updated_at = ?1 WHERE is_pinned = 0 AND deleted = 0",
                params![now],
            )
            .map_err(|e| e.to_string())?;
        Ok(affected)
    }

    // ---- Snippets CRUD ----

    pub fn list_snippets(&self) -> Result<Vec<Snippet>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, trigger_key, title, body, category, enabled, use_count, updated_at
                 FROM snippets
                 WHERE deleted = 0
                 ORDER BY category ASC, trigger_key ASC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Snippet {
                    id: row.get(0)?,
                    trigger: row.get(1)?,
                    title: row.get(2)?,
                    body: row.get(3)?,
                    category: row.get(4)?,
                    enabled: row.get::<_, i64>(5)? != 0,
                    use_count: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| e.to_string())?);
        }
        Ok(list)
    }

    pub fn save_snippet(
        &self,
        id: Option<String>,
        trigger: &str,
        title: &str,
        body: &str,
        category: &str,
        enabled: bool,
    ) -> Result<Snippet, String> {
        let trigger_trimmed = trigger.trim();
        if trigger_trimmed.is_empty() {
            return Err("El atajo (trigger) del snippet no puede estar vacío".into());
        }
        let now = Utc::now().timestamp_millis();
        let snippet_id = id
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO snippets (id, trigger_key, title, body, category, enabled, use_count, updated_at, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, 0)
             ON CONFLICT(id) DO UPDATE SET
                trigger_key = excluded.trigger_key,
                title = excluded.title,
                body = excluded.body,
                category = excluded.category,
                enabled = excluded.enabled,
                updated_at = excluded.updated_at,
                deleted = 0",
            params![
                snippet_id,
                trigger_trimmed,
                title.trim(),
                body,
                if category.trim().is_empty() {
                    "General"
                } else {
                    category.trim()
                },
                if enabled { 1 } else { 0 },
                now
            ],
        )
        .map_err(|e| format!("Error guardando snippet (¿trigger duplicado?): {e}"))?;

        Ok(Snippet {
            id: snippet_id,
            trigger: trigger_trimmed.to_string(),
            title: title.trim().to_string(),
            body: body.to_string(),
            category: if category.trim().is_empty() {
                "General".to_string()
            } else {
                category.trim().to_string()
            },
            enabled,
            use_count: 0,
            updated_at: now,
        })
    }

    pub fn increment_snippet_use(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE snippets SET use_count = use_count + 1 WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_snippet(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE snippets SET deleted = 1, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    // ---- Sync Helpers (Last-Write-Wins) ----

    pub fn get_all_clips_for_sync(&self) -> Result<Vec<(ClipboardItem, bool)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, category, content, preview, content_hash, language,
                        image_path, image_data_url, byte_size, is_pinned,
                        source_device, created_at, updated_at, copy_count, deleted
                 FROM clipboard_items",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    ClipboardItem {
                        id: row.get(0)?,
                        category: ClipCategory::from_str(&row.get::<_, String>(1)?),
                        content: row.get(2)?,
                        preview: row.get(3)?,
                        content_hash: row.get(4)?,
                        language: row.get(5)?,
                        image_path: row.get(6)?,
                        image_data_url: row.get(7)?,
                        byte_size: row.get::<_, i64>(8)? as usize,
                        is_pinned: row.get::<_, i64>(9)? != 0,
                        source_device: row.get(10)?,
                        created_at: row.get(11)?,
                        updated_at: row.get(12)?,
                        copy_count: row.get(13)?,
                    },
                    row.get::<_, i64>(14)? != 0,
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    pub fn get_all_snippets_for_sync(&self) -> Result<Vec<(Snippet, bool)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, trigger_key, title, body, category, enabled, use_count, updated_at, deleted
                 FROM snippets",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    Snippet {
                        id: row.get(0)?,
                        trigger: row.get(1)?,
                        title: row.get(2)?,
                        body: row.get(3)?,
                        category: row.get(4)?,
                        enabled: row.get::<_, i64>(5)? != 0,
                        use_count: row.get(6)?,
                        updated_at: row.get(7)?,
                    },
                    row.get::<_, i64>(8)? != 0,
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    pub fn merge_remote_clip(&self, item: &ClipboardItem, deleted: bool) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let existing: Option<(String, i64, i64)> = conn
            .query_row(
                "SELECT id, updated_at, deleted FROM clipboard_items WHERE id = ?1 OR content_hash = ?2 LIMIT 1",
                params![item.id, item.content_hash],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        if let Some((existing_id, local_ts, local_deleted)) = existing {
            if local_ts >= item.updated_at && (local_deleted != 0) == deleted {
                return Ok(false);
            }
            conn.execute(
                "UPDATE clipboard_items SET
                    category = ?1,
                    content = ?2,
                    preview = ?3,
                    language = ?4,
                    image_path = COALESCE(?5, image_path),
                    image_data_url = COALESCE(?6, image_data_url),
                    is_pinned = ?7,
                    source_device = ?8,
                    updated_at = ?9,
                    copy_count = ?10,
                    deleted = ?11
                 WHERE id = ?12",
                params![
                    item.category.as_str(),
                    item.content,
                    item.preview,
                    item.language,
                    item.image_path,
                    item.image_data_url,
                    if item.is_pinned { 1 } else { 0 },
                    item.source_device,
                    item.updated_at,
                    item.copy_count,
                    if deleted { 1 } else { 0 },
                    existing_id
                ],
            )
            .map_err(|e| e.to_string())?;
            return Ok(true);
        }

        if deleted {
            return Ok(false);
        }

        conn.execute(
            "INSERT INTO clipboard_items (
                id, category, content, preview, content_hash, language,
                image_path, image_data_url, byte_size, is_pinned,
                source_device, created_at, updated_at, copy_count, deleted
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 0)",
            params![
                item.id,
                item.category.as_str(),
                item.content,
                item.preview,
                item.content_hash,
                item.language,
                item.image_path,
                item.image_data_url,
                item.byte_size as i64,
                if item.is_pinned { 1 } else { 0 },
                item.source_device,
                item.created_at,
                item.updated_at,
                item.copy_count,
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(true)
    }

    pub fn merge_remote_snippet(&self, snippet: &Snippet, deleted: bool) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let local_updated: Option<i64> = conn
            .query_row(
                "SELECT updated_at FROM snippets WHERE id = ?1 OR trigger_key = ?2 LIMIT 1",
                params![snippet.id, snippet.trigger],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        if let Some(local_ts) = local_updated {
            if local_ts >= snippet.updated_at {
                return Ok(false);
            }
        }

        conn.execute(
            "INSERT INTO snippets (id, trigger_key, title, body, category, enabled, use_count, updated_at, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(trigger_key) DO UPDATE SET
                title = excluded.title,
                body = excluded.body,
                category = excluded.category,
                enabled = excluded.enabled,
                updated_at = excluded.updated_at,
                deleted = excluded.deleted",
            params![
                snippet.id,
                snippet.trigger,
                snippet.title,
                snippet.body,
                snippet.category,
                if snippet.enabled { 1 } else { 0 },
                snippet.use_count,
                snippet.updated_at,
                if deleted { 1 } else { 0 }
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(true)
    }
}

