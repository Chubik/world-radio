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
        runCatching {
            val raw = json.encodeToString(
                ListSerializer(FavStation.serializer()),
                stations.map { FavStation.of(it) },
            )
            // write-then-rename so a process killed mid-write never leaves a
            // half-file where a reader can see it. if rename fails (e.g. because
            // the destination exists), delete the destination first and retry.
            // this ensures the write is still atomic: the temp file is either
            // successfully renamed to replace the destination, or not consumed at all.
            tmp.writeText(raw)
            val renamed = tmp.renameTo(file) || (file.delete() && tmp.renameTo(file))
            when {
                !renamed -> Log.w("r4dio", "catalog cache write failed: could not rename temp file")
            }
        }.onFailure {
            Log.w("r4dio", "catalog cache write failed: ${it.message}")
        }.also {
            // ensure the temp file is always cleaned up, even if an exception was thrown
            // or if both rename attempts failed
            tmp.delete()
        }
    }
}
