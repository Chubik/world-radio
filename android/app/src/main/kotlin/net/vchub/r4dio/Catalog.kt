package net.vchub.r4dio

import kotlinx.serialization.json.Json
import okhttp3.FormBody
import okhttp3.OkHttpClient
import okhttp3.Request
import kotlin.random.Random

private val EXCLUDED_COUNTRYCODES = setOf("RU", "BY")
private val EXCLUDED_NAME_SUBSTRINGS = listOf(
    "russia", "russian", "moscow", "moskva", "kremlin", "putin",
    "россия", "русск", "москв", "kreml",
    "беларус", "belarus", "минск", "minsk",
)

const val DEFAULT_TARGET = 1000

// ask for half again as many as we keep, so banned and hidden-country stations
// do not eat into the target. expressed as a ratio because `3 / 2` as a single
// integer constant would evaluate to 1.
private const val OVERFETCH_NUMERATOR = 3
private const val OVERFETCH_DENOMINATOR = 2

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

// the api can include a country but not exclude one, so exclusions are applied
// here — over-fetching is what keeps the kept list a full `target` afterwards.
fun takeAllowed(
    stations: List<Station>,
    userExcluded: Set<String>,
    target: Int,
): List<Station> = stations.filter { allowedStation(it, userExcluded) }.take(target)

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

class Catalog(
    private val client: OkHttpClient = OkHttpClient(),
    private val baseUrl: String = "https://all.api.radio-browser.info",
) {
    private val json = Json { ignoreUnknownKeys = true }

    fun fetchStations(
        target: Int = DEFAULT_TARGET,
        userExcluded: Set<String> = emptySet(),
    ): List<Station> {
        val ask = target * OVERFETCH_NUMERATOR / OVERFETCH_DENOMINATOR
        repeat(2) {
            val result = runCatching { fetchOnce(ask) }.getOrDefault(emptyList())
            if (result.isNotEmpty()) return takeAllowed(result, userExcluded, target)
        }
        val last = runCatching { fetchOnce(ask) }.getOrDefault(emptyList())
        return takeAllowed(last, userExcluded, target)
    }

    /**
     * resolves specific stations by id. the catalogue is the top-1000 by clickcount,
     * so a favourite starred on the desktop is usually absent from it — this is the
     * only way to get its stream url. user-excluded countries are deliberately NOT
     * applied: an explicit favourite outranks a country filter, same as pickFav.
     */
    fun fetchByUuids(uuids: List<String>): List<Station> {
        if (uuids.isEmpty()) return emptyList()
        val body = FormBody.Builder().add("uuids", uuids.joinToString(",")).build()
        val request = Request.Builder()
            .url("$baseUrl/json/stations/byuuid")
            .header("User-Agent", "world-radio-android/1.0")
            .post(body)
            .build()
        return runCatching {
            client.newCall(request).execute().use { resp ->
                val text = resp.body?.string().orEmpty()
                if (!resp.isSuccessful || text.isBlank()) return emptyList()
                json.decodeFromString<List<ApiStation>>(text)
                    .map { it.toStation() }
                    .filter { allowedStation(it) }
            }
        }.getOrDefault(emptyList())
    }

    private fun fetchOnce(limit: Int): List<Station> {
        val url =
            "$baseUrl/json/stations/search" +
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
