package com.ulticp.android

import android.annotation.SuppressLint
import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.view.ViewGroup
import android.view.Window
import android.view.WindowInsets
import android.webkit.JavascriptInterface
import android.webkit.WebChromeClient
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
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

        repository = SyncthingRepository(applicationContext)
        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

        val rootContainer = FrameLayout(this).apply {
            setBackgroundColor(Color.parseColor("#0B0F19"))
            fitsSystemWindows = false
            setOnApplyWindowInsetsListener { view, insets ->
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                    val bars = insets.getInsets(
                        WindowInsets.Type.systemBars() or
                                WindowInsets.Type.displayCutout() or
                                WindowInsets.Type.ime()
                    )
                    view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
                } else {
                    @Suppress("DEPRECATION")
                    view.setPadding(
                        insets.systemWindowInsetLeft,
                        insets.systemWindowInsetTop,
                        insets.systemWindowInsetRight,
                        insets.systemWindowInsetBottom
                    )
                }
                insets
            }
        }

        webView = WebView(this).apply {
            setBackgroundColor(Color.parseColor("#0B0F19"))
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            webChromeClient = WebChromeClient()
            webViewClient = WebViewClient()
            addJavascriptInterface(AndroidBridge(), "UltiCPBridge")
            loadUrl("file:///android_asset/index.html")
        }

        rootContainer.addView(
            webView,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        )

        setContentView(rootContainer)
        rootContainer.requestApplyInsets()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) {
            captureSystemClipboardIfPresent()
            repository.syncAndExportNow()
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
            repository.syncAndExportNow()
            runOnUiThread {
                Toast.makeText(this@MainActivity, "Portapapeles capturado y sincronizado", Toast.LENGTH_SHORT).show()
            }
            return repository.toStateJson()
        }

        @JavascriptInterface
        fun syncNow(): String {
            repository.syncAndExportNow()
            runOnUiThread {
                Toast.makeText(this@MainActivity, "Sincronizado con carpeta Syncthing", Toast.LENGTH_SHORT).show()
            }
            return repository.toStateJson()
        }

        @JavascriptInterface
        fun requestAllFilesPermission() {
            runOnUiThread {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                    try {
                        val intent = Intent(
                            Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                            Uri.parse("package:$packageName")
                        )
                        startActivity(intent)
                    } catch (_: Exception) {
                        startActivity(Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION))
                    }
                }
            }
        }

        @JavascriptInterface
        fun addClip(text: String, pinned: Boolean): String {
            val trimmed = text.trim()
            if (trimmed.isNotEmpty()) {
                repository.recordCopiedText(trimmed, pinned)
                runOnUiThread {
                    clipboardManager.setPrimaryClip(ClipData.newPlainText("UltiCP", trimmed))
                    Toast.makeText(this@MainActivity, "Guardado y exportado a Syncthing", Toast.LENGTH_SHORT).show()
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
            repository.syncAndExportNow()
            runOnUiThread {
                Toast.makeText(this@MainActivity, "Sincronizado correctamente", Toast.LENGTH_SHORT).show()
            }
            return repository.toStateJson()
        }
    }
}
