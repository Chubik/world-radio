package net.vchub.r4dio

import android.util.Log
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import java.io.File

private const val CACHE_FILE = "catalog.json"

const val CATALOG_TTL_SECS = 86_400L

/**
 * a delta may never remove more than this share of what is held: beyond it the
 * answer is a bug, not a day's news, and the caller downloads everything
 * instead. the same guard exists in the rust cache.
 */
private const val MAX_DELTA_REMOVAL_SHARE = 0.5

// syncedAt of 0 means "never synced", which is always stale — the subtraction
// handles that without a special case.
fun catalogIsStale(syncedAt: Long, now: Long, ttlSecs: Long = CATALOG_TTL_SECS): Boolean =
    now - syncedAt >= ttlSecs

/**
 * the station catalogue on disk. deliberately a plain file rather than datastore:
 * datastore keeps its whole contents in memory and rewrites the file on every
 * edit, which a ~173kb catalogue would make expensive for unrelated settings.
 */
class CatalogCache(private val dir: File) {
    private val json = Json { ignoreUnknownKeys = true }
    private val file get() = File(dir, CACHE_FILE)

    // the service holds a single instance, so an instance lock serialises every
    // production reader and writer against each other.
    private val lock = Any()

    fun read(): List<Station> = synchronized(lock) { readLocked() }

    private fun readLocked(): List<Station> {
        // a leftover .bak can only come from a version of the app that used a
        // move-aside write. it is never the only copy now, so just clear it.
        File(dir, "$CACHE_FILE.bak").delete()
        if (!file.exists()) {
            return emptyList()
        }
        return runCatching {
            json.decodeFromString(ListSerializer(FavStation.serializer()), file.readText())
                .map { it.toStation() }
        }.getOrElse {
            Log.w("r4dio", "catalog cache unreadable, refetching")
            emptyList()
        }
    }

    /**
     * folds [incoming] into what is already held and returns how many were
     * genuinely new. the whole read-modify-write sits inside the lock: a country
     * fetch and the background top-up both merge from background threads, and
     * outside the lock their read-modify-writes would overwrite each other.
     *
     * a station already held wins over its incoming copy, so a merge can never
     * rewrite a url under a listener, and never shrinks the catalogue.
     */
    fun merge(incoming: List<Station>): Int = synchronized(lock) {
        if (incoming.isEmpty()) {
            return 0
        }
        val held = readLocked()
        val heldUuids = held.map { it.uuid }.toSet()
        val fresh = incoming.filter { it.uuid !in heldUuids }.distinctBy { it.uuid }
        if (fresh.isEmpty()) {
            return 0
        }
        when (writeLocked(held + fresh)) {
            true -> fresh.size
            // the catalogue on disk is unchanged, so reporting a gain would let a
            // caller log or display stations that were never stored.
            false -> 0
        }
    }

    /** returns true only when the new content is on disk under [CACHE_FILE]. */
    fun write(stations: List<Station>): Boolean = synchronized(lock) { writeLocked(stations) }

    /**
     * applies a delta to the stored catalogue and returns the result, or null if
     * it could not be applied — in which case the caller falls back to a full
     * download and nothing on disk has been touched.
     */
    fun applyDelta(added: List<Station>, removed: Set<String>): List<Station>? =
        synchronized(lock) {
            val held = readLocked()
            if (held.isEmpty()) return@synchronized null
            if (removed.size.toDouble() / held.size > MAX_DELTA_REMOVAL_SHARE) {
                return@synchronized null
            }
            val byId = LinkedHashMap<String, Station>(held.size + added.size)
            held.forEach { byId[it.uuid] = it }
            removed.forEach { byId.remove(it) }
            // added last, so a station that is both removed and re-added survives.
            added.forEach { byId[it.uuid] = it }
            val merged = byId.values.toList()
            if (!writeLocked(merged)) return@synchronized null
            merged
        }

    /**
     * true when the held catalogue predates genres entirely. one-off: as soon as
     * a refetch lands, at least one station carries tags and this stops firing.
     */
    fun needsGenreBackfill(stations: List<Station>): Boolean =
        stations.isNotEmpty() && stations.none { it.tags.isNotBlank() }

    private fun writeLocked(stations: List<Station>): Boolean {
        var tmp: File? = null
        return runCatching {
            val raw = json.encodeToString(
                ListSerializer(FavStation.serializer()),
                stations.map { FavStation.of(it) },
            )
            // a unique temp file plus one rename: concurrent writers can never
            // share a temp path, and rename within a directory is atomic, so the
            // cache file is never absent and a reader never sees a half-file.
            val t = File.createTempFile("catalog", ".tmp", dir)
            tmp = t
            t.writeText(raw)
            val renamed = t.renameTo(file)
            if (!renamed) {
                Log.w("r4dio", "catalog cache write failed: could not rename temp file")
            }
            renamed
        }.getOrElse {
            Log.w("r4dio", "catalog cache write failed: ${it.message}")
            false
        }.also {
            // the temp file is never the only copy, so dropping it is always safe.
            tmp?.delete()
        }
    }
}
