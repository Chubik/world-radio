package net.vchub.r4dio

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
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

/**
 * what a data: line on the account stream can be. anything else — an unknown
 * event type from a newer server, a keep-alive — parses to null and is dropped,
 * which is what keeps this build working against any server.
 */
sealed class StreamEvent {
    data class Play(val event: MirrorEvent) : StreamEvent()
    object ProfileChanged : StreamEvent()
}

/**
 * the doorbell debounce, kept here so it can be exercised on its own: it claims
 * the slot only when no re-sync is already queued. the caller releases it when
 * that sync has finished, so a burst collapses into one.
 */
object ResyncGate {
    fun claim(queued: java.util.concurrent.atomic.AtomicBoolean): Boolean =
        queued.compareAndSet(false, true)

    fun release(queued: java.util.concurrent.atomic.AtomicBoolean) = queued.set(false)
}

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

    fun events(key: String, onEvent: (StreamEvent) -> Unit) {
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
                            val evt = parseStreamEvent(line)
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

    fun parseSseData(line: String): MirrorEvent? =
        (parseStreamEvent(line) as? StreamEvent.Play)?.event

    fun parseStreamEvent(line: String): StreamEvent? {
        val trimmed = line.removePrefix("data:").trim()
        return when (line.startsWith("data:")) {
            false -> null
            true -> {
                val obj = runCatching {
                    json.parseToJsonElement(trimmed) as? JsonObject
                }.getOrNull() ?: return null
                val type = (obj["type"] as? JsonPrimitive)?.contentOrNull
                when (type) {
                    "profile_changed" -> StreamEvent.ProfileChanged
                    // an unknown type is dropped rather than guessed at.
                    null -> runCatching {
                        json.decodeFromString(MirrorEvent.serializer(), trimmed)
                    }.getOrNull()?.let { StreamEvent.Play(it) }
                    else -> null
                }
            }
        }
    }
}
