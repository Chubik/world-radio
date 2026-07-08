package net.vchub.r4dio

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody

@Serializable
data class MirrorEvent(
    val uuid: String,
    val name: String,
    val url: String,
    val origin: String,
    val seq: Long,
)

@Serializable
private data class PlayBody(val uuid: String, val name: String, val url: String, val origin: String)

class MirrorClient(
    private val baseUrl: String = "https://r4dio.net",
    private val client: OkHttpClient = OkHttpClient(),
) {
    private val json = Json { ignoreUnknownKeys = true }
    private val jsonType = "application/json".toMediaType()

    fun play(key: String, uuid: String, name: String, url: String, origin: String): Long? {
        val payload = json.encodeToString(
            PlayBody.serializer(),
            PlayBody(uuid, name, url, origin),
        )
        val req = Request.Builder()
            .url("$baseUrl/play")
            .header("Authorization", "Bearer $key")
            .post(payload.toRequestBody(jsonType))
            .build()
        return runCatching {
            client.newCall(req).execute().use { resp ->
                when (resp.isSuccessful) {
                    false -> null
                    true -> {
                        val body = resp.body?.string().orEmpty()
                        json.decodeFromString<Map<String, Long>>(body)["seq"]
                    }
                }
            }
        }.getOrNull()
    }

    fun events(key: String, onEvent: (MirrorEvent) -> Unit) {
        val req = Request.Builder()
            .url("$baseUrl/events")
            .header("Authorization", "Bearer $key")
            .get()
            .build()
        runCatching {
            client.newCall(req).execute().use { resp ->
                when (resp.isSuccessful) {
                    false -> {}
                    true -> {
                        val source = resp.body?.source() ?: return
                        while (true) {
                            val line = source.readUtf8Line() ?: break
                            val evt = parseSseData(line)
                            when (evt) {
                                null -> {}
                                else -> onEvent(evt)
                            }
                        }
                    }
                }
            }
        }
    }

    fun parseSseData(line: String): MirrorEvent? {
        val trimmed = line.removePrefix("data:").trim()
        return when (line.startsWith("data:")) {
            false -> null
            true -> runCatching { json.decodeFromString(MirrorEvent.serializer(), trimmed) }.getOrNull()
        }
    }
}
