package net.vchub.r4dio

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody

@Serializable
data class SyncData(
    val favs: List<String>,
    val blocked: List<String>,
    @kotlinx.serialization.SerialName("excluded_countries")
    val excluded_countries: List<String> = emptyList(),
)

class SyncClient(
    private val baseUrl: String = "https://r4dio.net",
    private val client: OkHttpClient = OkHttpClient(),
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
