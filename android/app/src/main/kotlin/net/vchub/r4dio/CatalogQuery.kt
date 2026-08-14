package net.vchub.r4dio

data class CatalogFilters(
    val countries: Set<String> = emptySet(),
    val genres: Set<String> = emptySet(),
    val codecs: Set<String> = emptySet(),
    val minBitrate: Int = 0,
) {
    val isEmpty: Boolean
        get() = countries.isEmpty() && genres.isEmpty() && codecs.isEmpty() && minBitrate <= 0

    val activeCount: Int
        get() = listOf(countries.isNotEmpty(), genres.isNotEmpty(), codecs.isNotEmpty(), minBitrate > 0)
            .count { it }
}

/**
 * how well a station answers a query, lower being better. the order is the whole
 * point: matching genre, language and country as well as the name finds far more
 * of what exists (measured on the live catalogue: "french" 26 names against 2,226
 * stations that actually broadcast in french), but without ranking a short query
 * gets buried — "us" matches 7,714 stations by country code alone.
 *
 * so breadth comes from matching every field, and usefulness comes from putting
 * name matches first.
 */
private const val RANK_NAME_PREFIX = 0
private const val RANK_NAME_ANYWHERE = 1
private const val RANK_GENRE = 2
private const val RANK_LANGUAGE = 3
private const val RANK_COUNTRY = 4
private const val RANK_NO_MATCH = 5

/**
 * genre, language and country match as whole values, never as substrings: a
 * two-letter country code compared loosely turns every query into a country
 * query, and "pop" inside "popular" is not a genre match. the name is the one
 * field where a substring is right, because that is how someone types half of a
 * name they half remember.
 */
internal fun matchRank(station: Station, needle: String): Int {
    val name = station.name.lowercase()
    if (name.startsWith(needle)) return RANK_NAME_PREFIX
    if (name.contains(needle)) return RANK_NAME_ANYWHERE
    if (station.genres().any { it == needle }) return RANK_GENRE
    if (station.languages().any { it == needle }) return RANK_LANGUAGE
    if (station.country.lowercase() == needle) return RANK_COUNTRY
    return RANK_NO_MATCH
}

/**
 * the ban is applied first and unconditionally: no query or filter combination
 * may surface a banned station. [allowedStation] is not used here — it also
 * folds in blocked-ness and the shuffle country filter, neither of which belongs
 * on a browse screen that must still show blocked stations to let them be
 * unblocked.
 *
 * with a query the result is ordered by [matchRank]; without one the catalogue's
 * own order is left alone, since that is upstream's popularity ordering and no
 * relevance score can improve on it.
 */
fun searchCatalog(stations: List<Station>, query: String, filters: CatalogFilters): List<Station> {
    val needle = query.trim().lowercase()
    val filtered = stations
        .asSequence()
        .filterNot { isExcluded(it) }
        .filter { filters.countries.isEmpty() || it.country.uppercase() in filters.countries }
        .filter { filters.genres.isEmpty() || it.genres().any { g -> g in filters.genres } }
        .filter { filters.codecs.isEmpty() || it.codec.uppercase() in filters.codecs }
        .filter { it.bitrate >= filters.minBitrate }
    if (needle.isEmpty()) {
        return filtered.toList()
    }
    // sortedBy is stable, so stations of equal rank keep the catalogue's order
    // rather than being shuffled by the sort itself.
    return filtered
        .map { it to matchRank(it, needle) }
        .filter { it.second != RANK_NO_MATCH }
        .sortedBy { it.second }
        .map { it.first }
        .toList()
}

private fun <T> facetsOf(stations: List<Station>, key: (Station) -> T): List<Pair<T, Int>> {
    val counts = LinkedHashMap<T, Int>()
    for (station in stations) {
        if (isExcluded(station)) continue
        val k = key(station)
        counts[k] = (counts[k] ?: 0) + 1
    }
    return counts.entries
        .map { it.key to it.value }
        .sortedWith(compareByDescending<Pair<T, Int>> { it.second }.thenBy { it.first.toString() })
}

fun countryFacets(stations: List<Station>): List<Pair<String, Int>> =
    facetsOf(stations) { it.country.uppercase() }

fun genreFacets(stations: List<Station>): List<Pair<String, Int>> {
    val counts = LinkedHashMap<String, Int>()
    for (station in stations) {
        if (isExcluded(station)) continue
        for (genre in station.genres()) {
            counts[genre] = (counts[genre] ?: 0) + 1
        }
    }
    return counts.entries
        .map { it.key to it.value }
        .sortedWith(compareByDescending<Pair<String, Int>> { it.second }.thenBy { it.first })
}

fun codecFacets(stations: List<Station>): List<Pair<String, Int>> =
    facetsOf(stations) { it.codec.uppercase() }
