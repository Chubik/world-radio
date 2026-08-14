package net.vchub.r4dio

import kotlinx.serialization.Serializable

/** what the library shows for one past play. */
@Serializable
data class HistoryEntry(
    val station: FavStation,
    val at: Long,
)

/**
 * how many plays are kept. a listener scrolling their own history wants the
 * recent ones; keeping every play a shuffle ever made would grow without bound
 * for no one's benefit.
 */
const val PLAY_HISTORY_CAP = 100

/**
 * the plays this phone has made, newest first.
 *
 * deliberately separate from the [HistoryQueue] that feeds sync: that one is a
 * push queue which **deletes** entries once the server accepts them, and stores
 * no station names — it empties itself on a linked account and shows nothing on
 * an unlinked one. this list is the opposite: it never leaves the device, keeps
 * the whole station so a row can be rendered and replayed offline, and does not
 * care whether an account exists.
 */
object PlayHistory {
    /**
     * a replay moves the station to the top rather than adding a second row: a
     * history listing the same station eleven times is a worse answer to "what
     * was that station" than one listing eleven different ones.
     */
    fun append(held: List<HistoryEntry>, station: Station, at: Long): List<HistoryEntry> {
        val fresh = HistoryEntry(FavStation.of(station), at)
        return (listOf(fresh) + held.filterNot { it.station.uuid == station.uuid })
            .sortedByDescending { it.at }
            .take(PLAY_HISTORY_CAP)
    }

    /** what the screen renders: the stations, newest play first. */
    fun stations(held: List<HistoryEntry>): List<Station> =
        held.sortedByDescending { it.at }.map { it.station.toStation() }
}

/**
 * the blocked list, rendered.
 *
 * only uuids are stored — the format the desktop and cli sync — so names come
 * from the catalogue. a station missing from it still gets a row, carrying its
 * uuid as the name: hiding it would make the block permanent, since this screen
 * is the only place it can be undone.
 *
 * favourites are searched too. a blocked station is usually gone from the
 * catalogue precisely because it was blocked out of a listing, and a starred one
 * keeps its full record in the fav cache.
 */
fun blockedStations(
    blocked: Set<String>,
    catalogue: List<Station>,
    favourites: List<Station> = emptyList(),
): List<Station> {
    if (blocked.isEmpty()) {
        return emptyList()
    }
    val known = HashMap<String, Station>(blocked.size)
    for (station in favourites + catalogue) {
        if (station.uuid in blocked) {
            known.putIfAbsent(station.uuid, station)
        }
    }
    return blocked
        .map { uuid ->
            known[uuid] ?: Station(
                uuid = uuid,
                // no name to show, and inventing one ("Unknown station") would
                // read as a real station rather than a stub.
                name = uuid,
                url = "",
                country = "",
                codec = "",
                bitrate = 0,
            )
        }
        .sortedBy { it.name.lowercase() }
}
