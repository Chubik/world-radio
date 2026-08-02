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
            // across the whole operation so if anything fails, the old cache survives.
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
            // ensure temp and backup files are always cleaned up, even if an exception
            // was thrown or if the rename failed. the real cache is either the new
            // content or the restored previous content.
            tmp.delete()
            bak.delete()
        }
    }
}
