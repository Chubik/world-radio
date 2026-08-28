package net.vchub.r4dio

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody

/** a last-write-wins field: [value] is opaque to the transport, [at] is the
 *  client-side unix time of the change. */
@Serializable
data class Lww(val value: JsonElement, val at: Long)

/** one play-history entry: [id] is the station uuid, [gone] marks a removal
 *  the same way favourites and blocked do. */
@OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)
@Serializable
data class HistoryRecord(
    val id: String,
    val at: Long,
    @kotlinx.serialization.EncodeDefault
    val gone: Boolean = false,
)

/**
 * the four profile fields are nullable/empty and never encoded when unset, so a
 * device that has never touched them sends a payload byte-identical to the
 * pre-profile format — see [SyncProfile.outgoing], which is the only place that
 * decides whether they are set.
 */
@OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)
@Serializable
data class SyncData(
    val favs: List<String>,
    val blocked: List<String>,
    // always on the wire, unlike the four below: the pre-profile payload carried
    // it unconditionally and the server's own client still does.
    @kotlinx.serialization.EncodeDefault
    @kotlinx.serialization.SerialName("excluded_countries")
    val excluded_countries: List<String> = emptyList(),
    @kotlinx.serialization.SerialName("shuffle_filter")
    val shuffle_filter: Lww? = null,
    val scope: Lww? = null,
    val theme: Lww? = null,
    val history: List<HistoryRecord> = emptyList(),
    // what this device removed since its last successful sync — see PendingChanges.
    // absent (not an empty object) when there is nothing to report, which is also
    // what a pre-tombstone client sends and the server already treats as a no-op.
    val changed: PendingChanges? = null,
)

class SyncClient(
    private val baseUrl: String = "https://r4dio.net",
    // creating an account is not idempotent — each POST /account mints a new key —
    // so disable connection-failure retries to avoid minting duplicate accounts.
    private val client: OkHttpClient = OkHttpClient.Builder()
        .retryOnConnectionFailure(false)
        .build(),
) {
    private val json = Json { ignoreUnknownKeys = true }
    private val jsonType = "application/json".toMediaType()

    fun createAccount(): String? {
        val req = Request.Builder().url("$baseUrl/account").post(ByteArray(0).toRequestBody()).build()
        return runCatching {
            client.newCall(req).execute().use { resp ->
                when (resp.isSuccessful) {
                    false -> null
                    true -> {
                        val body = resp.body?.string().orEmpty()
                        json.decodeFromString<Map<String, String>>(body)["key"]
                    }
                }
            }
        }.getOrNull()
    }

    fun pull(key: String): SyncData? {
        val req = Request.Builder().url("$baseUrl/sync").header("Authorization", "Bearer $key").get().build()
        return execData(req)
    }

    fun push(key: String, data: SyncData): SyncData? {
        val body = json.encodeToString(SyncData.serializer(), data).toRequestBody(jsonType)
        val req = Request.Builder().url("$baseUrl/sync").header("Authorization", "Bearer $key").put(body).build()
        return execData(req)
    }

    fun delete(key: String): Boolean {
        val req = Request.Builder().url("$baseUrl/account").header("Authorization", "Bearer $key").delete().build()
        return runCatching {
            client.newCall(req).execute().use { it.isSuccessful }
        }.getOrDefault(false)
    }

    private fun execData(req: Request): SyncData? =
        runCatching {
            client.newCall(req).execute().use { resp ->
                when (resp.isSuccessful) {
                    false -> null
                    true -> json.decodeFromString(SyncData.serializer(), resp.body?.string().orEmpty())
                }
            }
        }.getOrNull()
}
