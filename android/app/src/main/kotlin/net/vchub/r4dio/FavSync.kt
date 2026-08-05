package net.vchub.r4dio

/**
 * favourites live in two places: the uuid set (which sync overwrites) and the
 * cached station objects (which playback picks from). sync only ever wrote the
 * first, so favourites starred on another device were never playable here.
 * these functions decide what the cache must become; the IO lives in the caller.
 */
object FavSync {
    fun missingUuids(wanted: Set<String>, known: List<Station>): List<String> {
        val have = known.map { it.uuid }.toSet()
        return wanted.filterNot { have.contains(it) }
    }

    fun reconcile(
        wanted: Set<String>,
        known: List<Station>,
        fetched: List<Station>,
    ): List<Station> {
        // known wins over fetched: a locally starred station already carries the
        // metadata the user saw, and re-fetching can return a renamed entry.
        val byUuid = LinkedHashMap<String, Station>()
        for (s in fetched) byUuid[s.uuid] = s
        for (s in known) byUuid[s.uuid] = s
        return wanted.mapNotNull { byUuid[it] }.filter { it.url.isNotBlank() }
    }
}
