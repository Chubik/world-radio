package net.vchub.r4dio

import kotlinx.serialization.json.Json
import okhttp3.OkHttpClient
import okhttp3.Request
import kotlin.random.Random

fun pickRandom(stations: List<Station>, rng: Random = Random.Default): Station? {
    val playable = stations.filter { it.url.isNotBlank() }
    if (playable.isEmpty()) return null
    return playable[rng.nextInt(playable.size)]
}

fun pickForScope(
    scope: Scope,
    catalog: List<Station>,
    favs: List<Station>,
    rng: Random = Random.Default,
): Station? =
    when (scope) {
        Scope.ALL -> pickRandom(catalog, rng)
        Scope.FAVS -> FavLogic.pickFav(favs, rng)
    }

class Catalog(private val client: OkHttpClient = OkHttpClient()) {
    private val json = Json { ignoreUnknownKeys = true }

    fun fetchStations(limit: Int = 200): List<Station> {
        val url =
            "https://all.api.radio-browser.info/json/stations/search" +
                "?limit=$limit&hidebroken=true&order=clickcount&reverse=true"
        val request = Request.Builder()
            .url(url)
            .header("User-Agent", "world-radio-android/1.0")
            .build()
        client.newCall(request).execute().use { resp ->
            val body = resp.body?.string().orEmpty()
            if (!resp.isSuccessful || body.isBlank()) return emptyList()
            val api = json.decodeFromString<List<ApiStation>>(body)
            return api.map { it.toStation() }.filter { it.url.isNotBlank() }
        }
    }
}
