package net.vchub.r4dio

import kotlinx.serialization.json.Json
import okhttp3.OkHttpClient
import okhttp3.Request
import kotlin.random.Random

private val EXCLUDED_COUNTRYCODES = setOf("RU", "BY")
private val EXCLUDED_NAME_SUBSTRINGS = listOf(
    "russia", "russian", "moscow", "moskva", "kremlin", "putin",
    "россия", "русск", "москв", "kreml",
    "беларус", "belarus", "минск", "minsk",
)

fun isExcluded(station: Station): Boolean {
    if (station.country.uppercase() in EXCLUDED_COUNTRYCODES) {
        return true
    }
    val haystack = station.name.lowercase()
    return EXCLUDED_NAME_SUBSTRINGS.any { haystack.contains(it) }
}

fun allowedStation(station: Station, userExcluded: Set<String> = emptySet()): Boolean =
    station.url.isNotBlank() &&
        !isExcluded(station) &&
        station.country.uppercase() !in userExcluded

fun pickRandom(
    stations: List<Station>,
    userExcluded: Set<String> = emptySet(),
    rng: Random = Random.Default,
): Station? {
    val playable = stations.filter { allowedStation(it, userExcluded) }
    if (playable.isEmpty()) return null
    return playable[rng.nextInt(playable.size)]
}

fun pickForScope(
    scope: Scope,
    catalog: List<Station>,
    favs: List<Station>,
    userExcluded: Set<String> = emptySet(),
    rng: Random = Random.Default,
): Station? =
    when (scope) {
        Scope.ALL -> pickRandom(catalog, userExcluded, rng)
        // in favs mode, fall back to the full catalog when there are no favourites
        // yet — otherwise shuffle would return null and playback would just stop.
        Scope.FAVS -> FavLogic.pickFav(favs, rng) ?: pickRandom(catalog, userExcluded, rng)
    }

class Catalog(private val client: OkHttpClient = OkHttpClient()) {
    private val json = Json { ignoreUnknownKeys = true }

    fun fetchStations(limit: Int = 1000): List<Station> {
        repeat(2) { attempt ->
            val result = runCatching { fetchOnce(limit) }.getOrDefault(emptyList())
            if (result.isNotEmpty()) return result
        }
        return runCatching { fetchOnce(limit) }.getOrDefault(emptyList())
    }

    private fun fetchOnce(limit: Int): List<Station> {
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
            return api.map { it.toStation() }.filter { allowedStation(it) }
        }
    }
}
