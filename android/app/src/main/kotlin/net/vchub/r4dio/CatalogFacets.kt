package net.vchub.r4dio

/**
 * the genres worth offering. the catalogue holds 11,238 distinct tag strings,
 * most of them station self-descriptions ("radio", "fm"), places ("méxico") or
 * noise — a most-common list would surface those instead of genres.
 */
val OFFERED_GENRES = listOf(
    "pop", "rock", "news", "classical", "dance", "hits", "talk", "oldies",
    "80s", "jazz", "90s", "electronic", "house", "country", "70s", "folk",
    "soul", "indie", "sports", "techno", "ambient", "religious", "metal",
    "blues", "lounge", "60s", "reggae", "hip hop", "chill", "latin",
)

/**
 * the genres offered as one-tap chips under the search field, so the common
 * cases need no trip through the modal sheet — material 3's own guidance is that
 * filter chips can sit directly beneath a search field.
 *
 * six because the row scrolls horizontally and a row nobody scrolls shows about
 * that many; they are the head of [OFFERED_GENRES], which is already ordered by
 * how much of the real catalogue each covers.
 */
val QUICK_GENRES = OFFERED_GENRES.take(6)

/**
 * the quick chips to show. a genre already chosen in the sheet is dropped from
 * the row: it is displayed there as an active chip with a ✕, and offering the
 * same genre twice in one row invites tapping one and being surprised by the
 * other.
 */
fun quickGenreChips(filters: CatalogFilters): List<String> =
    QUICK_GENRES.filterNot { it in filters.genres }

/** the bitrate steps the sheet offers. 0 is "any". */
val BITRATE_STEPS = listOf(0, 64, 128, 192, 256, 320)

/**
 * the codec strings worth offering. the cache also carries "UNKNOWN" (1,965
 * stations, which is an absence rather than a choice) and video muxes like
 * "AAC,H.264" — a row a user picks expecting a codec and gets 67 video streams
 * from is worse than no row.
 */
val OFFERED_CODECS = setOf("MP3", "AAC", "AAC+", "OGG", "FLAC")

/**
 * how many country rows the sheet offers. the catalogue carries 240 country
 * codes and 125 of them hold fewer than 20 stations; listing them all buries
 * the genre and codec groups under a scroll nobody finishes. the commonest are
 * what a person actually reaches for, and search covers the rest.
 */
const val COUNTRY_ROW_CAP = 30

/** the country rows the sheet shows: the commonest first, capped, with empties
 *  dropped so no dead row appears. */
fun offeredCountryRows(facets: List<Pair<String, Int>>): List<Pair<String, Int>> =
    facets.filter { it.second > 0 }.take(COUNTRY_ROW_CAP)

/** the codec rows the sheet shows, keeping the facet order (commonest first)
 *  and dropping anything the catalogue has none of. */
fun offeredCodecRows(facets: List<Pair<String, Int>>): List<Pair<String, Int>> =
    facets.filter { it.first in OFFERED_CODECS && it.second > 0 }

/**
 * the genre rows the sheet shows: the curated list in its own order, each with
 * its live count, and any the current catalogue has none of dropped so no dead
 * row appears.
 */
fun offeredGenreRows(facets: List<Pair<String, Int>>): List<Pair<String, Int>> {
    val counts = facets.toMap()
    return OFFERED_GENRES.mapNotNull { genre ->
        val n = counts[genre] ?: return@mapNotNull null
        if (n <= 0) null else genre to n
    }
}

/**
 * one chip per value in force, in the order the sheet lists the groups. each
 * carries the token that removes it, so the chip row needs no per-group branch.
 */
data class FilterChip(val label: String, val group: FilterGroup, val value: String)

enum class FilterGroup { COUNTRY, GENRE, CODEC, BITRATE }

fun activeChips(filters: CatalogFilters): List<FilterChip> {
    val chips = mutableListOf<FilterChip>()
    filters.countries.sorted().forEach { chips += FilterChip(it, FilterGroup.COUNTRY, it) }
    filters.genres.sorted().forEach { chips += FilterChip(it.uppercase(), FilterGroup.GENRE, it) }
    filters.codecs.sorted().forEach { chips += FilterChip(it, FilterGroup.CODEC, it) }
    if (filters.minBitrate > 0) {
        chips += FilterChip("≥${filters.minBitrate}k", FilterGroup.BITRATE, filters.minBitrate.toString())
    }
    return chips
}

/** removes exactly what a chip's ✕ stands for, leaving every other filter alone. */
fun withoutChip(filters: CatalogFilters, chip: FilterChip): CatalogFilters = when (chip.group) {
    FilterGroup.COUNTRY -> filters.copy(countries = filters.countries - chip.value)
    FilterGroup.GENRE -> filters.copy(genres = filters.genres - chip.value)
    FilterGroup.CODEC -> filters.copy(codecs = filters.codecs - chip.value)
    FilterGroup.BITRATE -> filters.copy(minBitrate = 0)
}

/**
 * a set toggle for the sheet's rows. the caller passes the value in the case the
 * facets emit — searchCatalog compares the station's uppercased country and
 * codec against these as they are, so a wrongly-cased value silently matches
 * nothing.
 */
fun toggleValue(current: Set<String>, value: String): Set<String> =
    when (value in current) {
        true -> current - value
        false -> current + value
    }

/**
 * the filter set, flattened so a rotation does not silently drop it.
 * CatalogFilters holds sets, which Bundle cannot carry — losing a filter on
 * rotation looks like the app forgot what the user asked for.
 */
fun filtersToList(f: CatalogFilters): List<Any> = listOf(
    f.countries.toList(),
    f.genres.toList(),
    f.codecs.toList(),
    f.minBitrate,
    // by name, not ordinal: reordering the enum would otherwise turn a saved
    // "sort by name" into something else.
    f.sort.name,
)

@Suppress("UNCHECKED_CAST")
fun filtersFromList(saved: List<Any>): CatalogFilters = CatalogFilters(
    countries = (saved[0] as List<String>).toSet(),
    genres = (saved[1] as List<String>).toSet(),
    codecs = (saved[2] as List<String>).toSet(),
    minBitrate = saved[3] as Int,
    // absent from anything saved before sorting existed, and an unknown name
    // means a downgrade — both are answered by the default rather than a crash.
    sort = (saved.getOrNull(4) as? String)
        ?.let { name -> SortOrder.entries.firstOrNull { it.name == name } }
        ?: SortOrder.POPULAR,
)
