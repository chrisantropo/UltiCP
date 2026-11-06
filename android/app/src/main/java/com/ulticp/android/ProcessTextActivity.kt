package com.ulticp.android

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import com.ulticp.android.cleaner.FormatCleaner
import com.ulticp.android.sync.SyncthingRepository

/**
 * Actividad transparente que se integra en el menú contextual nativo de selección de texto de Android
 * ("Copiar / Pegar / UltiCP ⚡ Expandir / Limpiar") mediante `ACTION_PROCESS_TEXT` y en el menú Compartir (`ACTION_SEND`).
 *
 * Ventaja clave: No requiere `BIND_ACCESSIBILITY_SERVICE`, por lo que Google Play Protect permite instalar
 * el APK sin ningún bloqueo de seguridad, y permite reemplazar abreviaturas (`;firma`, `;fecha`, etc.) o
 * limpiar URLs de tracking directamente dentro de cualquier cuadro de texto en WhatsApp, Gmail, Chrome, etc.
 */
class ProcessTextActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val repository = SyncthingRepository(applicationContext)
        val clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

        val currentClip = runCatching {
            clipboardManager.primaryClip?.takeIf { it.itemCount > 0 }
                ?.getItemAt(0)?.coerceToText(this)?.toString()
        }.getOrNull()

        when (intent?.action) {
            Intent.ACTION_PROCESS_TEXT -> {
                val rawInput = intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)?.toString() ?: ""
                val isReadOnly = intent.getBooleanExtra(Intent.EXTRA_PROCESS_TEXT_READONLY, false)

                if (rawInput.isNotBlank()) {
                    val transformed = processSnippetOrClean(rawInput, currentClip, repository)

                    // Copy to clipboard and record in Syncthing history
                    clipboardManager.setPrimaryClip(ClipData.newPlainText("UltiCP", transformed))
                    repository.recordCopiedText(transformed)

                    if (!isReadOnly) {
                        // Replace the selected text directly inside the target app's input field!
                        val resultIntent = Intent().apply {
                            putExtra(Intent.EXTRA_PROCESS_TEXT, transformed)
                        }
                        setResult(RESULT_OK, resultIntent)
                        Toast.makeText(this, "UltiCP: Texto expandido / limpiado", Toast.LENGTH_SHORT).show()
                    } else {
                        Toast.makeText(this, "UltiCP: Copiado limpio al portapapeles", Toast.LENGTH_SHORT).show()
                    }
                }
            }

            Intent.ACTION_SEND -> {
                val sharedText = intent.getStringExtra(Intent.EXTRA_TEXT) ?: ""
                if (sharedText.isNotBlank()) {
                    val transformed = processSnippetOrClean(sharedText, currentClip, repository)
                    clipboardManager.setPrimaryClip(ClipData.newPlainText("UltiCP", transformed))
                    repository.recordCopiedText(transformed)
                    Toast.makeText(this, "UltiCP: Guardado y copiado limpio", Toast.LENGTH_SHORT).show()
                }
            }
        }

        finish()
    }

    private fun processSnippetOrClean(
        input: String,
        currentClip: String?,
        repository: SyncthingRepository
    ): String {
        val trimmed = input.trim()

        // 1. Check if input contains or matches any snippet trigger (e.g. ;firma, ;reunion, ;fecha)
        val (_, snippets) = repository.loadMergedState()
        var textWithSnippets = trimmed
        var expandedAny = false
        for (snip in snippets) {
            if (!snip.deleted && snip.enabled && snip.trigger.isNotEmpty() && textWithSnippets.contains(snip.trigger)) {
                val rendered = repository.renderSnippetBody(snip.body, currentClip)
                textWithSnippets = textWithSnippets.replace(snip.trigger, rendered)
                expandedAny = true
            }
        }
        if (expandedAny) {
            return textWithSnippets
        }

        // 2. Otherwise clean tracking URLs, prettify JSON, or normalize Markdown
        return FormatCleaner.transformPipeline(
            trimmed,
            cleanUrls = true,
            prettifyJson = true,
            minifyJson = false,
            cleanMarkdown = true
        ).first
    }
}

