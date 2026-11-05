package com.ulticp.android.sync

import android.content.Context
import android.os.Build
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.security.MessageDigest
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.UUID

data class ClipItemModel(
    val id: String,
    val category: String,
    val content: String,
    val preview: String,
    val contentHash: String,
    val language: String?,
    val byteSize: Int,
    val isPinned: Boolean,
    val sourceDevice: String,
    val createdAt: Long,
    val updatedAt: Long,
    val copyCount: Long,
    val deleted: Boolean = false
)

data class SnippetModel(
    val id: String,
    val trigger: String,
    val title: String,
    val body: String,
    val category: String,
    val enabled: Boolean,
    val useCount: Long,
    val updatedAt: Long,
    val deleted: Boolean = false
)

class SyncthingRepository(private val context: Context) {

    private val prefs = context.getSharedPreferences("ulticp_prefs", Context.MODE_PRIVATE)

    val deviceId: String
        get() = prefs.getString("device_id", null)
            ?: ("android-" + UUID.randomUUID().toString().take(8)).also {
                prefs.edit().putString("device_id", it).apply()
            }

    var deviceName: String
        get() = prefs.getString("device_name", null)
            ?: "Android-${Build.MODEL.replace(" ", "")}"
        set(value) {
            prefs.edit().putString("device_name", value).apply()
        }

    var autoCleanUrls: Boolean
        get() = prefs.getBoolean("auto_clean_urls", true)
        set(value) {
            prefs.edit().putBoolean("auto_clean_urls", value).apply()
        }

    var snippetExpansionEnabled: Boolean
        get() = prefs.getBoolean("snippet_expansion_enabled", true)
        set(value) {
            prefs.edit().putBoolean("snippet_expansion_enabled", value).apply()
        }

    init {
        seedDefaultsIfNeeded()
    }

    fun getSyncFolder(): File {
        val customPath = prefs.getString("sync_folder", null)
        return if (!customPath.isNullOrBlank()) {
            File(customPath)
        } else {
            File(context.getExternalFilesDir(null), "syncthing_shared")
        }.apply {
            File(this, "devices").mkdirs()
            File(this, "blobs").mkdirs()
        }
    }

    fun setSyncFolderPath(path: String) {
        prefs.edit().putString("sync_folder", path.trim()).apply()
        getSyncFolder()
    }

    private fun seedDefaultsIfNeeded() {
        val devicesDir = File(getSyncFolder(), "devices")
        val myFile = File(devicesDir, "$deviceId.jsonl")
        if (myFile.exists() && myFile.length() > 0L) return

        val now = System.currentTimeMillis()
        val starterSnippets = listOf(
            SnippetModel(
                id = UUID.randomUUID().toString(),
                trigger = ";firma",
                title = "Firma de correo profesional",
                body = "Un cordial saludo,\n\nEquipo de Ingeniería\nFecha: {{date}} ({{time}})",
                category = "Trabajo",
                enabled = true,
                useCount = 1,
                updatedAt = now
            ),
            SnippetModel(
                id = UUID.randomUUID().toString(),
                trigger = ";reunion",
                title = "Plantilla de notas de reunión",
                body = "## Notas de Reunión ({{date}})\n\n### Acuerdos\n- \n\n### Próximos pasos\n- [ ] ",
                category = "Productividad",
                enabled = true,
                useCount = 0,
                updatedAt = now
            ),
            SnippetModel(
                id = UUID.randomUUID().toString(),
                trigger = ";fecha",
                title = "Fecha y hora actual ISO",
                body = "{{datetime}}",
                category = "Utilidades",
                enabled = true,
                useCount = 2,
                updatedAt = now
            ),
            SnippetModel(
                id = UUID.randomUUID().toString(),
                trigger = ";movil",
                title = "Respuesta rápida desde Android",
                body = "Enviado desde mi Android sincronizado vía Syncthing P2P el {{date}} a las {{time}}.",
                category = "Móvil",
                enabled = true,
                useCount = 1,
                updatedAt = now
            )
        )

        val starterClips = listOf(
            createClipModel(
                "Bienvenido a UltiCP Android — Sincroniza tu portapapeles y snippets con Windows mediante Syncthing P2P y expande abreviaturas en cualquier app.",
                pinned = true
            ),
            createClipModel(
                "https://github.com/syncthing/syncthing?utm_source=newsletter&utm_medium=social&fbclid=IwAR12345",
                pinned = false
            ),
            createClipModel(
                "{\n  \"app\": \"UltiCP\",\n  \"platform\": \"Android\",\n  \"sync\": \"Syncthing JSONL\"\n}",
                pinned = false
            )
        )

        saveAllState(starterClips, starterSnippets)
    }

    private fun classifyText(text: String): Pair<String, String?> {
        val trimmed = text.trim()
        if (!trimmed.contains(" ") && !trimmed.contains("\n") &&
            (trimmed.startsWith("http://") || trimmed.startsWith("https://"))
        ) {
            return Pair("url", null)
        }
        if ((trimmed.startsWith("{") && trimmed.endsWith("}")) ||
            (trimmed.startsWith("[") && trimmed.endsWith("]"))
        ) {
            return Pair("code", "json")
        }
        if (trimmed.contains("fun ") || trimmed.contains("const ") ||
            trimmed.contains("SELECT ") || trimmed.contains("function ")
        ) {
            return Pair("code", "code")
        }
        return Pair("plain_text", null)
    }

    private fun createClipModel(text: String, pinned: Boolean): ClipItemModel {
        val trimmed = text.trim()
        val now = System.currentTimeMillis()
        val (cat, lang) = classifyText(trimmed)
        return ClipItemModel(
            id = UUID.randomUUID().toString(),
            category = cat,
            content = trimmed,
            preview = trimmed.replace("\n", " ↵ ").take(180),
            contentHash = sha256Hex(trimmed.toByteArray()),
            language = lang,
            byteSize = trimmed.toByteArray().size,
            isPinned = pinned,
            sourceDevice = deviceName,
            createdAt = now,
            updatedAt = now,
            copyCount = 1,
            deleted = false
        )
    }

    fun loadMergedState(): Pair<MutableList<ClipItemModel>, MutableList<SnippetModel>> {
        val devicesDir = File(getSyncFolder(), "devices")
        val clipsByHash = linkedMapOf<String, ClipItemModel>()
        val snippetsByTrigger = linkedMapOf<String, SnippetModel>()

        devicesDir.listFiles { f -> f.extension == "jsonl" }?.forEach { file ->
            file.forEachLine { line ->
                val trimmed = line.trim()
                if (trimmed.isEmpty()) return@forEachLine
                runCatching {
                    val obj = JSONObject(trimmed)
                    val deleted = obj.optBoolean("deleted", false)
                    when (obj.optString("record_type")) {
                        "clip" -> {
                            val item = obj.getJSONObject("item")
                            val clip = ClipItemModel(
                                id = item.getString("id"),
                                category = item.optString("category", "plain_text"),
                                content = item.getString("content"),
                                preview = item.optString("preview", item.getString("content").take(180)),
                                contentHash = item.getString("content_hash"),
                                language = if (item.isNull("language")) null else item.optString("language"),
                                byteSize = item.optInt("byte_size", 0),
                                isPinned = item.optBoolean("is_pinned", false),
                                sourceDevice = item.optString("source_device", "Dispositivo"),
                                createdAt = item.optLong("created_at", 0L),
                                updatedAt = item.optLong("updated_at", 0L),
                                copyCount = item.optLong("copy_count", 1L),
                                deleted = deleted
                            )
                            val existing = clipsByHash[clip.contentHash]
                            if (existing == null || clip.updatedAt >= existing.updatedAt) {
                                clipsByHash[clip.contentHash] = clip
                            }
                        }
                        "snippet" -> {
                            val s = obj.getJSONObject("snippet")
                            val snippet = SnippetModel(
                                id = s.getString("id"),
                                trigger = s.getString("trigger"),
                                title = s.getString("title"),
                                body = s.getString("body"),
                                category = s.optString("category", "General"),
                                enabled = s.optBoolean("enabled", true),
                                useCount = s.optLong("use_count", 0L),
                                updatedAt = s.optLong("updated_at", 0L),
                                deleted = deleted
                            )
                            val existing = snippetsByTrigger[snippet.trigger]
                            if (existing == null || snippet.updatedAt >= existing.updatedAt) {
                                snippetsByTrigger[snippet.trigger] = snippet
                            }
                        }
                    }
                }
            }
        }

        val clips = clipsByHash.values.toMutableList()
        clips.sortWith(compareByDescending<ClipItemModel> { it.isPinned }.thenByDescending { it.updatedAt })
        val snippets = snippetsByTrigger.values.toMutableList()
        snippets.sortBy { it.trigger }
        return Pair(clips, snippets)
    }

    private fun saveAllState(clips: List<ClipItemModel>, snippets: List<SnippetModel>) {
        val devicesDir = File(getSyncFolder(), "devices")
        devicesDir.mkdirs()
        val myFile = File(devicesDir, "$deviceId.jsonl")
        val sb = StringBuilder()

        for (c in clips) {
            val itemJson = JSONObject().apply {
                put("id", c.id)
                put("category", c.category)
                put("content", c.content)
                put("preview", c.preview)
                put("content_hash", c.contentHash)
                put("language", c.language ?: JSONObject.NULL)
                put("image_path", JSONObject.NULL)
                put("image_data_url", JSONObject.NULL)
                put("byte_size", c.byteSize)
                put("is_pinned", c.isPinned)
                put("source_device", c.sourceDevice)
                put("created_at", c.createdAt)
                put("updated_at", c.updatedAt)
                put("copy_count", c.copyCount)
            }
            val rec = JSONObject().apply {
                put("record_type", "clip")
                put("item", itemJson)
                put("deleted", c.deleted)
            }
            sb.append(rec.toString()).append("\n")
        }

        for (s in snippets) {
            val snipJson = JSONObject().apply {
                put("id", s.id)
                put("trigger", s.trigger)
                put("title", s.title)
                put("body", s.body)
                put("category", s.category)
                put("enabled", s.enabled)
                put("use_count", s.useCount)
                put("updated_at", s.updatedAt)
            }
            val rec = JSONObject().apply {
                put("record_type", "snippet")
                put("snippet", snipJson)
                put("deleted", s.deleted)
            }
            sb.append(rec.toString()).append("\n")
        }

        myFile.writeText(sb.toString())
    }

    fun recordCopiedText(text: String, pinned: Boolean = false): ClipItemModel {
        val trimmed = text.trim()
        val hash = sha256Hex(trimmed.toByteArray())
        val (clips, snippets) = loadMergedState()
        val now = System.currentTimeMillis()

        val idx = clips.indexOfFirst { it.contentHash == hash }
        val updatedClip = if (idx >= 0) {
            val old = clips[idx]
            old.copy(
                updatedAt = now,
                copyCount = old.copyCount + 1,
                isPinned = old.isPinned || pinned,
                deleted = false
            ).also { clips[idx] = it }
        } else {
            createClipModel(trimmed, pinned).also { clips.add(0, it) }
        }

        saveAllState(clips, snippets)
        return updatedClip
    }

    fun togglePin(id: String): Boolean {
        val (clips, snippets) = loadMergedState()
        val idx = clips.indexOfFirst { it.id == id }
        if (idx < 0) return false
        val nextPin = !clips[idx].isPinned
        clips[idx] = clips[idx].copy(isPinned = nextPin, updatedAt = System.currentTimeMillis())
        saveAllState(clips, snippets)
        return nextPin
    }

    fun deleteClip(id: String) {
        val (clips, snippets) = loadMergedState()
        val idx = clips.indexOfFirst { it.id == id }
        if (idx >= 0) {
            clips[idx] = clips[idx].copy(deleted = true, updatedAt = System.currentTimeMillis())
            saveAllState(clips, snippets)
        }
    }

    fun saveSnippet(trigger: String, title: String, body: String, category: String): SnippetModel {
        val (clips, snippets) = loadMergedState()
        val now = System.currentTimeMillis()
        val cleanTrigger = trigger.trim()
        val idx = snippets.indexOfFirst { it.trigger == cleanTrigger }
        val model = if (idx >= 0) {
            snippets[idx].copy(
                title = title.trim(),
                body = body,
                category = category.trim().ifEmpty { "General" },
                updatedAt = now,
                deleted = false
            ).also { snippets[idx] = it }
        } else {
            SnippetModel(
                id = UUID.randomUUID().toString(),
                trigger = cleanTrigger,
                title = title.trim(),
                body = body,
                category = category.trim().ifEmpty { "General" },
                enabled = true,
                useCount = 0,
                updatedAt = now,
                deleted = false
            ).also { snippets.add(it) }
        }
        saveAllState(clips, snippets)
        return model
    }

    fun deleteSnippet(id: String) {
        val (clips, snippets) = loadMergedState()
        val idx = snippets.indexOfFirst { it.id == id }
        if (idx >= 0) {
            snippets[idx] = snippets[idx].copy(deleted = true, updatedAt = System.currentTimeMillis())
            saveAllState(clips, snippets)
        }
    }

    fun renderSnippetBody(body: String, currentClipboard: String?): String {
        val now = Date()
        val dateStr = SimpleDateFormat("yyyy-MM-dd", Locale.getDefault()).format(now)
        val timeStr = SimpleDateFormat("HH:mm", Locale.getDefault()).format(now)
        val dateTimeStr = SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.getDefault()).format(now)

        return body
            .replace("{{date}}", dateStr)
            .replace("{{time}}", timeStr)
            .replace("{{datetime}}", dateTimeStr)
            .replace("{{uuid}}", UUID.randomUUID().toString())
            .replace("{{clipboard}}", currentClipboard ?: "")
    }

    fun tryExpandSuffixSnippet(currentText: String, currentClipboard: String?): String? {
        if (!snippetExpansionEnabled) return null
        val (_, snippets) = loadMergedState()
        val match = snippets.firstOrNull {
            !it.deleted && it.enabled && it.trigger.isNotEmpty() && currentText.endsWith(it.trigger)
        } ?: return null

        val rendered = renderSnippetBody(match.body, currentClipboard)
        return currentText.removeSuffix(match.trigger) + rendered
    }

    fun toStateJson(): String {
        val (clips, snippets) = loadMergedState()
        val clipsArr = JSONArray()
        clips.filter { !it.deleted }.forEach { c ->
            clipsArr.put(JSONObject().apply {
                put("id", c.id)
                put("category", c.category)
                put("content", c.content)
                put("preview", c.preview)
                put("language", c.language ?: "")
                put("byte_size", c.byteSize)
                put("is_pinned", c.isPinned)
                put("source_device", c.sourceDevice)
                put("updated_at", c.updatedAt)
                put("copy_count", c.copyCount)
            })
        }

        val snippetsArr = JSONArray()
        snippets.filter { !it.deleted }.forEach { s ->
            snippetsArr.put(JSONObject().apply {
                put("id", s.id)
                put("trigger", s.trigger)
                put("title", s.title)
                put("body", s.body)
                put("category", s.category)
                put("use_count", s.useCount)
            })
        }

        return JSONObject().apply {
            put("clips", clipsArr)
            put("snippets", snippetsArr)
            put("device_id", deviceId)
            put("device_name", deviceName)
            put("sync_folder", getSyncFolder().absolutePath)
            put("auto_clean_urls", autoCleanUrls)
            put("snippet_expansion_enabled", snippetExpansionEnabled)
        }.toString()
    }

    private fun sha256Hex(bytes: ByteArray): String {
        val digest = MessageDigest.getInstance("SHA-256").digest(bytes)
        return digest.joinToString("") { "%02x".format(it) }
    }
}
