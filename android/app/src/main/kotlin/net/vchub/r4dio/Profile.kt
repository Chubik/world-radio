package net.vchub.r4dio

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/** how many plays the queue and the server both keep, newest by `at` first. */
const val HISTORY_CAP = 200

/**
 * the wire word for a scope android can show. the five words are the published
 * contract shared with the desktop — see radio-core's sync/scope.rs.
 */
fun wireScope(scope: Scope): String = when (scope) {
    Scope.ALL -> "all"
    Scope.FAVS -> "favorites"
}

/**
 * null for anything this build cannot show, so the caller leaves the local scope
 * alone. approximating `recent`/`blocked`/`dead` as ALL and re-publishing it
 * would drag the desktop off the scope the user chose there. the uppercase words
 * are legacy values written by clients already in the wild.
 */
fun localScope(wire: String): Scope? = when (wire) {
    "all", "ALL" -> Scope.ALL
    "favorites", "FAVS" -> Scope.FAVS
    else -> null
}

/**
 * the listening profile as it is stored and synced: the shuffle-filter countries,
 * the browse scope and the theme, each with the client time of the change so a
 * round-trip can take the newer side rather than blindly overwriting.
 *
 * the `At` stamps are taken when the user changes something, never at sync time —
 * a sync-time stamp would always outrank the other device. a stamp of 0 means the
 * field was never touched here, and the field is then omitted from the payload
 * entirely.
 */
data class SyncProfile(
    val countries: List<String> = emptyList(),
    val countriesAt: Long = 0,
    val scope: String = "",
    val scopeAt: Long = 0,
    val theme: String = "",
    val themeAt: Long = 0,
) {
    /**
     * the one stamping rule, used by every local change. a same-value save must not
     * move the stamp, or an idle device would outrank a device that actually changed
     * something on the next merge.
     *
     * only the scope has a local editor on android: the filter and the theme are
     * chosen on the desktop and only arrive through [applyRemote].
     */
    fun withScope(next: String, now: Long): SyncProfile {
        if (next == scope) return this
        return copy(scope = next, scopeAt = now)
    }

    fun outgoing(
        favs: List<String>,
        blocked: List<String>,
        excluded: List<String>,
        plays: List<HistoryRecord>,
    ): SyncData = SyncData(
        favs = favs,
        blocked = blocked,
        excluded_countries = excluded,
        shuffle_filter = when (countriesAt) {
            0L -> null
            else -> Lww(
                buildJsonObject {
                    put("countries", JsonArray(countries.map { JsonPrimitive(it) }))
                },
                countriesAt,
            )
        },
        scope = stringLww(scope, scopeAt),
        theme = stringLww(theme, themeAt),
        history = plays,
    )

    /** takes each field the server sent back that is newer than the local stamp. */
    fun applyRemote(server: SyncData): SyncProfile {
        var out = this
        val filter = remoteCountries(server.shuffle_filter)
        if (filter != null && filter.second > out.countriesAt) {
            out = out.copy(countries = filter.first, countriesAt = filter.second)
        }
        val scope = remoteString(server.scope)
        if (scope != null && scope.second > out.scopeAt) {
            out = out.copy(scope = scope.first, scopeAt = scope.second)
        }
        val theme = remoteString(server.theme)
        if (theme != null && theme.second > out.themeAt) {
            out = out.copy(theme = theme.first, themeAt = theme.second)
        }
        return out
    }

    private fun stringLww(value: String, at: Long): Lww? = when (at) {
        0L -> null
        else -> Lww(JsonPrimitive(value), at)
    }

    private fun remoteCountries(lww: Lww?): Pair<List<String>, Long>? {
        if (lww == null) return null
        val list = runCatching {
            lww.value.jsonObject["countries"]?.jsonArray?.mapNotNull { it.jsonPrimitive.contentOrNull }
        }.getOrNull() ?: return null
        return list.map { it.uppercase() } to lww.at
    }

    private fun remoteString(lww: Lww?): Pair<String, Long>? {
        if (lww == null) return null
        val value = runCatching { lww.value.jsonPrimitive.contentOrNull }.getOrNull() ?: return null
        return value to lww.at
    }
}

/**
 * the plays waiting to be pushed. stored as `uuid|at` strings because DataStore
 * has a string-set type and no list-of-object type. android has no history ui
 * yet, so the merged history that comes back is deliberately not stored: this
 * queue exists only to feed the other devices.
 */
object HistoryQueue {
    fun append(queued: Set<String>, uuid: String, at: Long): Set<String> {
        val existing = records(queued).associateBy { it.id }.toMutableMap()
        val have = existing[uuid]
        // a replay pushes the newer time rather than a second entry, and an
        // out-of-order stamp never overwrites a newer one — same rule the server
        // merges by, so the two cannot disagree.
        if (have == null || at > have.at) {
            existing[uuid] = HistoryRecord(uuid, at, false)
        }
        return existing.values
            .sortedByDescending { it.at }
            .take(HISTORY_CAP)
            .map { "${it.id}|${it.at}" }
            .toSet()
    }

    fun records(queued: Set<String>): List<HistoryRecord> = queued
        .mapNotNull { entry ->
            val cut = entry.lastIndexOf('|')
            if (cut <= 0) return@mapNotNull null
            val at = entry.substring(cut + 1).toLongOrNull() ?: return@mapNotNull null
            HistoryRecord(entry.substring(0, cut), at, false)
        }
        .sortedByDescending { it.at }

    /** removes exactly what was pushed, so a play recorded during the round-trip
     *  survives instead of being dropped by a blanket clear. */
    fun drain(queued: Set<String>, pushed: List<HistoryRecord>): Set<String> =
        queued - pushed.map { "${it.id}|${it.at}" }.toSet()
}
