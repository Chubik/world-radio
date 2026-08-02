package net.vchub.r4dio

import android.util.Log
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import java.io.File

private const val CACHE_FILE = "catalog.json"

/**
 * the station catalogue on disk. deliberately a plain file rather than datastore:
 * datastore keeps its whole contents in memory and rewrites the file on every
 * edit, which a ~173kb catalogue would make expensive for unrelated settings.
 */
class CatalogCache(private val dir: File) {
    private val json = Json { ignoreUnknownKeys = true }
    private val file get() = File(dir, CACHE_FILE)

    fun read(): List<Station> {
        val bak = File(dir, "$CACHE_FILE.bak")
        // if the cache is missing but a backup exists, the backup is the only
        // surviving copy (from a failed write). promote it to the cache location.
        if (!file.exists() && bak.exists()) {
            bak.renameTo(file)
        }
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

    fun write(stations: List<Station>) {
        val tmp = File(dir, "$CACHE_FILE.tmp")
        val bak = File(dir, "$CACHE_FILE.bak")
        runCatching {
            val raw = json.encodeToString(
                ListSerializer(FavStation.serializer()),
                stations.map { FavStation.of(it) },
            )
            // write-then-rename so a process killed mid-write never leaves a
            // half-file where a reader can see it. preserve the previous cache
            // across the whole operation: on failure, the backup survives and
            // read() will recover it. only delete the backup once a cache file exists.
            tmp.writeText(raw)
            bak.delete()
            // move the current cache aside before trying to replace it. if this
            // succeeds, we have a backup to restore. if it fails, there was no
            // cache yet, which is fine.
            val hadPreviousCache = file.renameTo(bak)
            // now try to put the new cache in place. if this fails, restore the backup.
            val renamed = tmp.renameTo(file)
            when {
                renamed -> Unit  // success: new cache is in place, backup will be cleaned
                hadPreviousCache -> {
                    // new cache failed to land, restore the backup
                    bak.renameTo(file)
                    Log.w("r4dio", "catalog cache write failed: could not complete rename, previous cache preserved")
                }
                else -> {
                    // new cache failed and there was no previous cache to restore
                    Log.w("r4dio", "catalog cache write failed: could not rename temp file")
                }
            }
        }.onFailure {
            Log.w("r4dio", "catalog cache write failed: ${it.message}")
        }.also {
            // clean up temp file unconditionally (it is never the only copy).
            // only delete backup if cache file exists, because if it does not exist,
            // the backup may be the only surviving copy from a failed restore.
            tmp.delete()
            if (file.exists()) {
                bak.delete()
            }
        }
    }
}
