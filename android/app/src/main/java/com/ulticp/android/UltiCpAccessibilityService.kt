package com.ulticp.android

import android.accessibilityservice.AccessibilityService
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Bundle
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import com.ulticp.android.cleaner.FormatCleaner
import com.ulticp.android.sync.SyncthingRepository

/**
 * Servicio de Accesibilidad para Android que permite:
 * 1. Expansión global de Snippets en cualquier app (WhatsApp, Gmail, Notas, Navegador)
 *    detectando abreviaturas como `;firma`, `;reunion`, `;fecha` al escribir.
 * 2. Limpieza automática de URLs de tracking y captura del portapapeles sincronizada
 *    con Windows mediante la carpeta compartida de Syncthing (`devices/<device_id>.jsonl`).
 */
class UltiCpAccessibilityService : AccessibilityService() {

    private lateinit var clipboardManager: ClipboardManager
    private lateinit var repository: SyncthingRepository
    private var isInternalClipboardWrite = false

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        if (isInternalClipboardWrite) return@OnPrimaryClipChangedListener
        val clip = clipboardManager.primaryClip ?: return@OnPrimaryClipChangedListener
        if (clip.itemCount == 0) return@OnPrimaryClipChangedListener
        val rawText = clip.getItemAt(0).coerceToText(this)?.toString()?.trim() ?: return@OnPrimaryClipChangedListener
        if (rawText.isEmpty()) return@OnPrimaryClipChangedListener

        val (cleanedText, removedCount) = FormatCleaner.cleanTrackingUrls(rawText)
        if (repository.autoCleanUrls && removedCount > 0 && cleanedText != rawText) {
            isInternalClipboardWrite = true
            clipboardManager.setPrimaryClip(ClipData.newPlainText("UltiCP Clean", cleanedText))
            isInternalClipboardWrite = false
            repository.recordCopiedText(cleanedText)
        } else {
            repository.recordCopiedText(rawText)
        }
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        repository = SyncthingRepository(applicationContext)
        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboardManager.addPrimaryClipChangedListener(clipListener)
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        if (event == null) return
        if (event.eventType == AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED) {
            val source = event.source ?: return
            val currentText = source.text?.toString() ?: return

            val currentClip = clipboardManager.primaryClip
                ?.takeIf { it.itemCount > 0 }
                ?.getItemAt(0)
                ?.coerceToText(this)
                ?.toString()

            val expanded = repository.tryExpandSuffixSnippet(currentText, currentClip)
            if (expanded != null && expanded != currentText) {
                val args = Bundle().apply {
                    putCharSequence(
                        AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,
                        expanded
                    )
                }
                source.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args)
            }
        }
    }

    override fun onInterrupt() {}

    override fun onDestroy() {
        if (::clipboardManager.isInitialized) {
            clipboardManager.removePrimaryClipChangedListener(clipListener)
        }
        super.onDestroy()
    }
}

