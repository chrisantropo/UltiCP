package com.ulticp.android.cleaner

import android.net.Uri
import org.json.JSONArray
import org.json.JSONObject

object FormatCleaner {
    private val TRACKING_KEYS = setOf(
        "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content",
        "utm_id", "utm_name", "fbclid", "gclid", "gclsrc", "dclid", "msclkid",
        "twclid", "ttclid", "yclid", "mc_cid", "mc_eid", "igshid", "igsh",
        "_hsenc", "_hsmi", "mkt_tok", "si", "spm", "ref_src", "ref_url"
    )

    private val URL_REGEX = Regex("""https?://[^\s<>()"'\]]+""")
    private val HEADING_REGEX = Regex("""^(\s*#{1,6})([^\s#].*)$""")
    private val BULLET_REGEX = Regex("""^(\s*)[\*\+]\s+(.*)$""")

    fun cleanTrackingUrls(input: String): Pair<String, Int> {
        var removedCount = 0
        val out = URL_REGEX.replace(input) { match ->
            val raw = match.value
            val trimmed = raw.trimEnd('.', ',', ';', ':', '!', '?')
            val suffix = raw.substring(trimmed.length)
            val (cleaned, count) = cleanSingleUrl(trimmed)
            removedCount += count
            cleaned + suffix
        }
        return Pair(out, removedCount)
    }

    private fun cleanSingleUrl(rawUrl: String): Pair<String, Int> {
        return try {
            val uri = Uri.parse(rawUrl)
            val queryNames = uri.queryParameterNames
            if (queryNames.isEmpty()) return Pair(rawUrl, 0)

            val builder = uri.buildUpon().clearQuery()
            var removed = 0
            for (name in queryNames) {
                val lower = name.lowercase()
                if (lower.startsWith("utm_") || lower.startsWith("hsa_") || TRACKING_KEYS.contains(lower)) {
                    removed++
                } else {
                    for (value in uri.getQueryParameters(name)) {
                        builder.appendQueryParameter(name, value)
                    }
                }
            }
            if (removed > 0) Pair(builder.build().toString(), removed) else Pair(rawUrl, 0)
        } catch (_: Exception) {
            Pair(rawUrl, 0)
        }
    }

    fun prettifyJsonIfValid(input: String): String? {
        val trimmed = input.trim()
        return try {
            when {
                trimmed.startsWith("{") && trimmed.endsWith("}") ->
                    JSONObject(trimmed).toString(2)
                trimmed.startsWith("[") && trimmed.endsWith("]") ->
                    JSONArray(trimmed).toString(2)
                else -> null
            }
        } catch (_: Exception) {
            null
        }
    }

    fun minifyJsonIfValid(input: String): String? {
        val trimmed = input.trim()
        return try {
            when {
                trimmed.startsWith("{") && trimmed.endsWith("}") ->
                    JSONObject(trimmed).toString()
                trimmed.startsWith("[") && trimmed.endsWith("]") ->
                    JSONArray(trimmed).toString()
                else -> null
            }
        } catch (_: Exception) {
            null
        }
    }

    fun formatMarkdown(input: String): String {
        val outLines = mutableListOf<String>()
        var inCodeBlock = false
        var emptyStreak = 0

        for (rawLine in input.lines()) {
            val trimmedEnd = rawLine.trimEnd()
            if (trimmedEnd.trimStart().startsWith("```")) {
                inCodeBlock = !inCodeBlock
                emptyStreak = 0
                outLines.add(trimmedEnd)
                continue
            }
            if (inCodeBlock) {
                outLines.add(rawLine)
                continue
            }
            if (trimmedEnd.isEmpty()) {
                emptyStreak++
                if (emptyStreak <= 1) outLines.add("")
                continue
            }
            emptyStreak = 0

            val hMatch = HEADING_REGEX.matchEntire(trimmedEnd)
            val bMatch = BULLET_REGEX.matchEntire(trimmedEnd)
            val normalized = when {
                hMatch != null -> "${hMatch.groupValues[1].trimStart()} ${hMatch.groupValues[2].trim()}"
                bMatch != null -> "${bMatch.groupValues[1]}- ${bMatch.groupValues[2]}"
                else -> trimmedEnd
            }
            outLines.add(normalized)
        }
        return outLines.joinToString("\n").trim()
    }

    fun transformPipeline(
        input: String,
        cleanUrls: Boolean,
        prettifyJson: Boolean,
        minifyJson: Boolean,
        cleanMarkdown: Boolean
    ): Pair<String, List<String>> {
        var current = input
        val actions = mutableListOf<String>()

        if (cleanUrls) {
            val (cleaned, count) = cleanTrackingUrls(current)
            if (count > 0 && cleaned != current) {
                current = cleaned
                actions.add("Eliminados $count parámetros de tracking")
            }
        }

        if (prettifyJson) {
            val pretty = prettifyJsonIfValid(current)
            if (pretty != null && pretty != current) {
                current = pretty
                actions.add("JSON formateado (legible)")
            }
        } else if (minifyJson) {
            val mini = minifyJsonIfValid(current)
            if (mini != null && mini != current) {
                current = mini
                actions.add("JSON minificado")
            }
        }

        if (cleanMarkdown && !prettifyJson && !minifyJson && prettifyJsonIfValid(current) == null) {
            val md = formatMarkdown(current)
            if (md != current) {
                current = md
                actions.add("Markdown normalizado")
            }
        }

        return Pair(current, actions)
    }
}
