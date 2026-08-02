package net.vchub.r4dio

import android.util.Log
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import java.io.File

private const val CACHE_FILE = "catalog.json"

const val CATALOG_TTL_SECS = 86_400L

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

    fun read(): List<Station> = synchronized(lock) {
        // a leftover .bak can only come from a version of the app that used a
        // move-aside write. it is never the only copy now, so just clear it.
        File(dir, "$CACHE_FILE.bak").delete()
        if (!file.exists()) {
            return emptyList()
        }
        runCatching {
            json.decodeFromString(ListSerializer(FavStation.serializer()), file.readText())
                .map { it.toStation() }
        }.getOrElse {
            Log.w("r4dio", "catalog cache unreadable, refetching")
            emptyList()
        }
    }

    /** returns true only when the new content is on disk under [CACHE_FILE]. */
    fun write(stations: List<Station>): Boolean = synchronized(lock) {
        var tmp: File? = null
        runCatching {
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
