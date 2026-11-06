package com.ulticp.android

import android.annotation.SuppressLint
import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Color
import android.os.Bundle
import android.view.Window
import android.webkit.JavascriptInterface
import android.webkit.WebChromeClient
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Toast
import com.ulticp.android.cleaner.FormatCleaner
import com.ulticp.android.sync.SyncthingRepository
import org.json.JSONArray
import org.json.JSONObject

class MainActivity : Activity() {

    private lateinit var webView: WebView
    private lateinit var repository: SyncthingRepository
    private lateinit var clipboardManager: ClipboardManager

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        requestWindowFeature(Window.FEATURE_NO_TITLE)
        window.statusBarColor = Color.parseColor("#0B0F19")
        window.navigationBarColor = Color.parseColor("#111827")

        repository = SyncthingRepository(applicationContext)
        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

        webView = WebView(this).apply {
            setBackgroundColor(Color.parseColor("#0B0F19"))
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            webChromeClient = WebChromeClient()
            webViewClient = WebViewClient()
            addJavascriptInterface(AndroidBridge(), "UltiCPBridge")
            loadUrl("file:///android_asset/index.html")
        }

        setContentView(webView)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) {
            // Android 10+ permits reading clipboard when window gains focus
            captureSystemClipboardIfPresent()
            webView.evaluateJavascript("window.refreshFromNative && window.refreshFromNative();", null)
        }
    }

    private fun captureSystemClipboardIfPresent(): String? {
        return try {
            val clip = clipboardManager.primaryClip ?: return null
            if (clip.itemCount == 0) return null
            val text = clip.getItemAt(0).coerceToText(this)?.toString()?.trim() ?: return null
            if (text.isEmpty()) return null

            val finalContent = if (repository.autoCleanUrls) {
                val (cleaned, count) = FormatCleaner.cleanTrackingUrls(text)
                if (count > 0 && cleaned != text) {
                    clipboardManager.setPrimaryClip(ClipData.newPlainText("UltiCP Clean", cleaned))
                    cleaned
                } else {
                    text
                }
            } else {
                text
            }

            repository.recordCopiedText(finalContent)
            finalContent
        } catch (_: Exception) {
            null
        }
    }

    inner class AndroidBridge {
        @JavascriptInterface
        fun getStateJson(): String {
            return repository.toStateJson()
        }

        @JavascriptInterface
        fun captureClipboardNow(): String {
            captureSystemClipboardIfPresent()
            return repository.toStateJson()
        }

        @JavascriptInterface
        fun addClip(text: String, pinned: Boolean): String {
            val trimmed = text.trim()
            if (trimmed.isNotEmpty()) {
                repository.recordCopiedText(trimmed, pinned)
                runOnUiThread {
                    clipboardManager.setPrimaryClip(ClipData.newPlainText("UltiCP", trimmed))
                }
            }
            return repository.toStateJson()
        }

        @JavascriptInterface
        fun copyClip(content: String, cleanFirst: Boolean): String {
            val finalStr = if (cleanFirst) {
                FormatCleaner.transformPipeline(content, true, true, false, true).first
            } else {
                content
            }
            repository.recordCopiedText(finalStr)
            runOnUiThread {
                clipboardManager.setPrimaryClip(ClipData.newPlainText("UltiCP", finalStr))
                Toast.makeText(
                    this@MainActivity,
                    if (cleanFirst) "Copiado limpio al portapapeles" else "Copiado al portapapeles",
                    Toast.LENGTH_SHORT
                ).show()
            }
            return repository.toStateJson()
        }

        @JavascriptInterface
        fun togglePin(id: String): String {
            repository.togglePin(id)
            return repository.toStateJson()
        }

        @JavascriptInterface
        fun deleteClip(id: String): String {
            repository.deleteClip(id)
            return repository.toStateJson()
        }

        @JavascriptInterface
        fun saveSnippet(trigger: String, title: String, body: String, category: String): String {
            if (trigger.isNotBlank() && body.isNotBlank()) {
                repository.saveSnippet(trigger, title, body, category)
            }
            return repository.toStateJson()
        }

        @JavascriptInterface
        fun deleteSnippet(id: String): String {
            repository.deleteSnippet(id)
            return repository.toStateJson()
        }

        @JavascriptInterface
        fun expandAndCopySnippet(body: String): String {
            val currentClip = runCatching {
                clipboardManager.primaryClip?.takeIf { it.itemCount > 0 }
                    ?.getItemAt(0)?.coerceToText(this@MainActivity)?.toString()
            }.getOrNull()

            val expanded = repository.renderSnippetBody(body, currentClip)
            repository.recordCopiedText(expanded)
            runOnUiThread {
                clipboardManager.setPrimaryClip(ClipData.newPlainText("UltiCP Snippet", expanded))
                Toast.makeText(this@MainActivity, "Snippet expandido y copiado", Toast.LENGTH_SHORT).show()
            }
            return expanded
        }

        @JavascriptInterface
        fun transformText(
            input: String,
            cleanUrls: Boolean,
            prettifyJson: Boolean,
            minifyJson: Boolean,
            cleanMarkdown: Boolean,
            copyOut: Boolean
        ): String {
            val (result, actions) = FormatCleaner.transformPipeline(
                input, cleanUrls, prettifyJson, minifyJson, cleanMarkdown
            )
            if (copyOut && result.isNotBlank()) {
                repository.recordCopiedText(result)
                runOnUiThread {
                    clipboardManager.setPrimaryClip(ClipData.newPlainText("UltiCP Clean", result))
                    Toast.makeText(this@MainActivity, "Resultado limpio copiado", Toast.LENGTH_SHORT).show()
                }
            }
            val actArr = JSONArray()
            actions.forEach { actArr.put(it) }
            return JSONObject().apply {
                put("result", result)
                put("actions", actArr)
            }.toString()
        }

        @JavascriptInterface
        fun updateSettings(
            deviceName: String,
            syncFolder: String,
            autoCleanUrls: Boolean,
            snippetExpansion: Boolean
        ): String {
            if (deviceName.isNotBlank()) repository.deviceName = deviceName.trim()
            if (syncFolder.isNotBlank()) repository.setSyncFolderPath(syncFolder.trim())
            repository.autoCleanUrls = autoCleanUrls
            repository.snippetExpansionEnabled = snippetExpansion
            return repository.toStateJson()
        }
    }
}

