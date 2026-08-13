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
 * the ban is applied first and unconditionally: no query or filter combination
 * may surface a banned station. [allowedStation] is not used here — it also
 * folds in blocked-ness and the shuffle country filter, neither of which belongs
 * on a browse screen that must still show blocked stations to let them be
 * unblocked.
 */
fun searchCatalog(stations: List<Station>, query: String, filters: CatalogFilters): List<Station> {
    val needle = query.trim().lowercase()
    return stations
        .asSequence()
        .filterNot { isExcluded(it) }
        .filter { needle.isEmpty() || it.name.lowercase().contains(needle) }
        .filter { filters.countries.isEmpty() || it.country.uppercase() in filters.countries }
        .filter { filters.genres.isEmpty() || it.genres().any { g -> g in filters.genres } }
        .filter { filters.codecs.isEmpty() || it.codec.uppercase() in filters.codecs }
        .filter { it.bitrate >= filters.minBitrate }
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
