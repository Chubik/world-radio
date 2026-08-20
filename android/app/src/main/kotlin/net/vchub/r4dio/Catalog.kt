package net.vchub.r4dio

import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import okhttp3.FormBody
import okhttp3.OkHttpClient
import okhttp3.Request
import kotlin.random.Random

/**
 * our own catalogue, not radio-browser's. see [Catalog.fetchCatalogue] for why
 * it exists at all.
 */
const val CATALOG_URL = "https://r4dio.net/catalog"

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

/**
 * which of the filtered countries still need fetching in full. the filter is the
 * moment the user's intent is explicit and narrow, so it is worth one request per
 * country — but only once per country per session, and never for a banned one.
 *
 * uppercased on both sides: the filter arrives from the desktop over the wire and
 * its case is not ours to trust.
 */
fun countriesToPull(filter: Set<String>, alreadyPulled: Set<String>): Set<String> {
    val pulled = alreadyPulled.map { it.uppercase() }.toSet()
    return filter
        .map { it.uppercase() }
        .filter { it.isNotBlank() && it !in pulled && it !in EXCLUDED_COUNTRYCODES }
        .toSet()
}

// measured against the live api on 2026-08-13: it holds 62,250 stations. the
// ceiling is the whole catalogue, not a guessed-at fraction of it.
const val TOP_UP_CEILING = 62_000

/**
 * above this many stations the catalogue counts as whole, and the pill stops
 * saying more is coming.
 *
 * deliberately below [TOP_UP_CEILING]: that number is what upstream holds
 * *before* our filtering, and the ban plus unplayable streams take ~3,300 of
 * them off. measured on the live api on 2026-08-13, a full fetch lands at
 * 58,938 — so comparing against the upstream figure would leave "+" showing
 * forever on a catalogue that is already complete.
 */
const val CATALOGUE_WHOLE = 50_000

/**
 * page size for [Catalog.fetchPage], which is now only the fallback path used
 * when our own catalogue server cannot be reached.
 */
const val TOP_UP_PAGE = 200

/**
 * whether now is a moment to fetch the catalogue. read at the moment of the
 * attempt, never cached: a phone on wi-fi a minute ago may be on mobile data in
 * a pocket now.
 *
 * the whole catalogue is one 4.3 mb request from our own server, so there is no
 * longer any reason to wait for a charger — that gate existed only because
 * walking radio-browser cost 69 mb.
 *
 * [dataSaver] outranks everything. when the user has asked android to hold back
 * background data, an app does not get to decide its own download is worth it.
 */
fun catalogueFetchAllowed(
    unmetered: Boolean,
    dataSaver: Boolean,
    onMobileAllowed: Boolean,
): Boolean {
    if (unmetered) {
        return true
    }
    return !dataSaver && onMobileAllowed
}

/**
 * [blocked] outranks everything, including a star: blocking is a pointed "never play
 * this station again", while excluding a country is a broad taste filter that an
 * explicit favourite is allowed to override (see fetchByUuids and FavLogic.pickFav).
 * That asymmetry is deliberate — do not collapse the two filters into one rule.
 *
 * [included] is the synced shuffle filter and belongs on the taste side: an empty
 * set means unrestricted, and a favourite outranks it exactly as it outranks
 * [userExcluded]. an excluded country still wins over an included one, so a country
 * the user hid cannot come back through the filter.
 */
fun allowedStation(
    station: Station,
    userExcluded: Set<String> = emptySet(),
    blocked: Set<String> = emptySet(),
    included: Set<String> = emptySet(),
): Boolean =
    station.url.isNotBlank() &&
        station.uuid !in blocked &&
        !isExcluded(station) &&
        station.country.uppercase() !in userExcluded &&
        (included.isEmpty() || station.country.uppercase() in included)

// the api can include a country but not exclude one, so exclusions are applied
// here — over-fetching is what keeps the kept list a full `target` afterwards.
fun takeAllowed(
    stations: List<Station>,
    userExcluded: Set<String>,
    target: Int,
    blocked: Set<String> = emptySet(),
): List<Station> = stations.filter { allowedStation(it, userExcluded, blocked) }.take(target)

fun pickRandom(
    stations: List<Station>,
    userExcluded: Set<String> = emptySet(),
    blocked: Set<String> = emptySet(),
    rng: Random = Random.Default,
    included: Set<String> = emptySet(),
): Station? {
    val playable = stations.filter { allowedStation(it, userExcluded, blocked, included) }
    if (playable.isEmpty()) return null
    return playable[rng.nextInt(playable.size)]
}

data class ScopePick(val station: Station?, val usedFallback: Boolean)

/**
 * in favs mode with no resolvable favourites we still play something from the
 * full catalogue — stopping dead is worse. but the caller needs to know it
 * happened: a silent fallback here is what made a broken favourites sync look
 * like normal shuffling under a FAVOURITES ONLY pill.
 */
fun pickForScopeDetailed(
    scope: Scope,
    catalog: List<Station>,
    favs: List<Station>,
    userExcluded: Set<String> = emptySet(),
    blocked: Set<String> = emptySet(),
    rng: Random = Random.Default,
    included: Set<String> = emptySet(),
): ScopePick =
    when (scope) {
        Scope.ALL -> ScopePick(pickRandom(catalog, userExcluded, blocked, rng, included), false)
        // the favs arm gets no filter — a star outranks a taste filter, same as it
        // outranks an excluded country. the fallback below is a catalogue pick, so
        // it does take the filter.
        Scope.FAVS -> when (val fav = FavLogic.pickFav(favs, blocked, rng)) {
            null -> ScopePick(pickRandom(catalog, userExcluded, blocked, rng, included), true)
            else -> ScopePick(fav, false)
        }
    }

fun pickForScope(
    scope: Scope,
    catalog: List<Station>,
    favs: List<Station>,
    userExcluded: Set<String> = emptySet(),
    blocked: Set<String> = emptySet(),
    rng: Random = Random.Default,
    included: Set<String> = emptySet(),
): Station? = pickForScopeDetailed(scope, catalog, favs, userExcluded, blocked, rng, included).station

/**
 * what a catalogue request came back with. an empty list used to mean both
 * "nothing changed" and "the download failed", and the caller could not tell
 * them apart — which is why this is a type rather than a list.
 */
sealed class CatalogueResult {
    object Unchanged : CatalogueResult()
    data class Fetched(val stations: List<Station>, val etag: String) : CatalogueResult()
    object Failed : CatalogueResult()
}

@Serializable
private data class WireDelta(
    val id: String,
    val added: List<FavStation> = emptyList(),
    val removed: List<String> = emptyList(),
)

/**
 * what the delta endpoint answered. `Unavailable` is not an error — it is the
 * server saying "you are too far behind for this", and the caller answers by
 * downloading everything, exactly as it did before deltas existed.
 */
sealed class DeltaResult {
    data class Changed(val added: List<Station>, val removed: Set<String>, val id: String) : DeltaResult()
    object Unchanged : DeltaResult()
    object Unavailable : DeltaResult()
}

class Catalog(
    private val client: OkHttpClient = OkHttpClient(),
    private val baseUrl: String = "https://all.api.radio-browser.info",
) {
    private val json = Json { ignoreUnknownKeys = true }

    fun fetchStations(
        target: Int = DEFAULT_TARGET,
        userExcluded: Set<String> = emptySet(),
        blocked: Set<String> = emptySet(),
    ): List<Station> {
        val ask = target * OVERFETCH_NUMERATOR / OVERFETCH_DENOMINATOR
        repeat(2) {
            val result = runCatching { fetchOnce(ask, blocked) }.getOrDefault(emptyList())
            if (result.isNotEmpty()) return takeAllowed(result, userExcluded, target, blocked)
        }
        val last = runCatching { fetchOnce(ask, blocked) }.getOrDefault(emptyList())
        return takeAllowed(last, userExcluded, target, blocked)
    }

    /**
     * resolves specific stations by id. the catalogue is the top-1000 by clickcount,
     * so a favourite starred on the desktop is usually absent from it — this is the
     * only way to get its stream url. user-excluded countries are deliberately NOT
     * applied: an explicit favourite outranks a country filter, same as pickFav.
     * [blocked] IS applied — a blocked station must not be resolvable into the fav
     * cache, because from there it would play without passing the shuffle filter.
     */
    fun fetchByUuids(uuids: List<String>, blocked: Set<String> = emptySet()): List<Station> {
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
                    .filter { allowedStation(it, blocked = blocked) }
            }
        }.getOrDefault(emptyList())
    }

    /**
     * every station in one country. the top-1000 by clickcount holds only a
     * handful of any country outside the big few — 7 of ukraine's 351 — so a
     * filter set to one is filtering almost nothing until this runs.
     */
    fun fetchCountry(code: String, blocked: Set<String> = emptySet()): List<Station> {
        val url = "$baseUrl/json/stations/bycountrycodeexact/$code?hidebroken=true"
        val request = Request.Builder()
            .url(url)
            .header("User-Agent", "world-radio-android/1.0")
            .build()
        return runCatching {
            client.newCall(request).execute().use { resp ->
                val body = resp.body?.string().orEmpty()
                if (!resp.isSuccessful || body.isBlank()) return emptyList()
                json.decodeFromString<List<ApiStation>>(body)
                    .map { it.toStation() }
                    .filter { allowedStation(it, blocked = blocked) }
            }
        }.getOrDefault(emptyList())
    }

    /**
     * one page further down the same clickcount ordering the first 1000 came from,
     * so the top-up keeps adding the next-most-popular stations rather than random
     * ones. the ban is applied here like on every other ingest path.
     */
    fun fetchPage(offset: Int, limit: Int, blocked: Set<String> = emptySet()): List<Station> {
        val url = "$baseUrl/json/stations/search" +
            "?limit=$limit&offset=$offset&hidebroken=true&order=clickcount&reverse=true"
        val request = Request.Builder()
            .url(url)
            .header("User-Agent", "world-radio-android/1.0")
            .build()
        return runCatching {
            client.newCall(request).execute().use { resp ->
                val body = resp.body?.string().orEmpty()
                if (!resp.isSuccessful || body.isBlank()) return emptyList()
                json.decodeFromString<List<ApiStation>>(body)
                    .map { it.toStation() }
                    .filter { allowedStation(it, blocked = blocked) }
            }
        }.getOrDefault(emptyList())
    }

    /**
     * the whole catalogue in one request, from our own server rather than
     * radio-browser. it exists because upstream sends 37 fields per station and
     * compresses nothing: the same ~59k stations are 69 mb from there and 4.3 mb
     * from here, which is what makes fetching on mobile data reasonable at all.
     *
     * the payload is already in the on-disk [FavStation] shape, so it needs no
     * transcoding — but the ban is still applied here. the server filters too;
     * this side does not get to assume that, because a stale or wrong server
     * must never be able to put a banned station in front of a listener.
     *
     * an empty result means failure, and every caller must treat it that way:
     * writing it to the cache would erase a good catalogue.
     */
    fun fetchCatalogue(
        url: String = CATALOG_URL,
        blocked: Set<String> = emptySet(),
    ): List<Station> {
        val request = Request.Builder()
            .url(url)
            .header("User-Agent", "world-radio-android/1.0")
            .build()
        return runCatching {
            client.newCall(request).execute().use { resp ->
                val body = resp.body?.string().orEmpty()
                if (!resp.isSuccessful || body.isBlank()) return emptyList()
                json.decodeFromString(ListSerializer(FavStation.serializer()), body)
                    .map { it.toStation() }
                    .filter { allowedStation(it, blocked = blocked) }
            }
        }.getOrDefault(emptyList())
    }

    /**
     * the incremental sibling of [fetchCatalogue]: sends the etag we already hold
     * so an unchanged catalogue costs a 304 instead of the full download. an empty
     * list used to mean both "nothing changed" and "the download failed", and the
     * caller could not tell them apart — which is why this returns [CatalogueResult]
     * instead.
     */
    fun fetchCatalogueResult(
        url: String = CATALOG_URL,
        etag: String = "",
        blocked: Set<String> = emptySet(),
    ): CatalogueResult {
        val builder = Request.Builder()
            .url(url)
            .header("User-Agent", "world-radio-android/1.0")
        if (etag.isNotBlank()) {
            builder.header("If-None-Match", etag)
        }
        return runCatching {
            client.newCall(builder.build()).execute().use { resp ->
                if (resp.code == 304) return@use CatalogueResult.Unchanged
                val body = resp.body?.string().orEmpty()
                if (!resp.isSuccessful || body.isBlank()) return@use CatalogueResult.Failed
                val stations = json
                    .decodeFromString(ListSerializer(FavStation.serializer()), body)
                    .map { it.toStation() }
                    .filter { allowedStation(it, blocked = blocked) }
                CatalogueResult.Fetched(stations, resp.header("ETag").orEmpty())
            }
        }.getOrDefault(CatalogueResult.Failed)
    }

    /**
     * what changed since [since], the etag of the snapshot already held — about
     * 100-150 kb against the 4.3 mb full catalogue. it is allowed to give up for
     * any reason at all ([DeltaResult.Unavailable]): a 409 (snapshot too old), a
     * 404 (a server from before deltas), any other failure, or no [since] to ask
     * against in the first place. the caller answers every one of those the same
     * way, by downloading everything.
     */
    fun fetchDelta(
        url: String = "$CATALOG_URL/delta",
        since: String,
        blocked: Set<String> = emptySet(),
    ): DeltaResult {
        if (since.isBlank()) return DeltaResult.Unavailable
        val request = Request.Builder()
            .url("$url?since=${java.net.URLEncoder.encode(since, "UTF-8")}")
            .header("User-Agent", "world-radio-android/1.0")
            .build()
        return runCatching {
            client.newCall(request).execute().use { resp ->
                when {
                    resp.code == 304 -> DeltaResult.Unchanged
                    // 409: snapshot too old for a delta. 404: a server from before
                    // deltas existed. both mean the same thing to the caller.
                    resp.code == 409 || resp.code == 404 -> DeltaResult.Unavailable
                    !resp.isSuccessful -> DeltaResult.Unavailable
                    else -> {
                        val body = resp.body?.string().orEmpty()
                        if (body.isBlank()) return@use DeltaResult.Unavailable
                        val wire = json.decodeFromString(WireDelta.serializer(), body)
                        DeltaResult.Changed(
                            added = wire.added.map { it.toStation() }
                                .filter { allowedStation(it, blocked = blocked) },
                            removed = wire.removed.toSet(),
                            id = wire.id,
                        )
                    }
                }
            }
        }.getOrDefault(DeltaResult.Unavailable)
    }

    private fun fetchOnce(limit: Int, blocked: Set<String>): List<Station> {
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
            return api.map { it.toStation() }.filter { allowedStation(it, blocked = blocked) }
        }
    }
}
