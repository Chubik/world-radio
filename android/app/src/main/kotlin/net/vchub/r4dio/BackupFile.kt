package net.vchub.r4dio

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * a portable copy of everything that cannot be recreated: the sync key (which IS
 * the account — there is no password to recover it with) plus the favourites and
 * filters. written through the system file picker, so it survives an uninstall
 * that takes the app's own storage with it.
 */
@Serializable
data class BackupPayload(
    // no defaults on these two: they are what identifies the file as ours, and a
    // default would let any well-formed json pass as a valid backup.
    val version: Int,
    val app: String,
    val key: String? = null,
    val favs: List<String> = emptyList(),
    val cached: List<FavStation> = emptyList(),
    val blocked: List<String> = emptyList(),
    val excluded: List<String> = emptyList(),
)

class Backup(
    val key: String?,
    val favs: Set<String>,
    val cached: List<Station>,
    val blocked: Set<String>,
    val excluded: Set<String>,
)

object BackupFile {
    const val VERSION = 1
    const val MARKER = "r4dio"

    private val json = Json { ignoreUnknownKeys = true; prettyPrint = true }

    fun encode(
        key: String?,
        favs: Set<String>,
        cached: List<Station>,
        blocked: Set<String>,
        excluded: Set<String>,
    ): String = json.encodeToString(
        BackupPayload.serializer(),
        BackupPayload(
            version = VERSION,
            app = MARKER,
            key = key,
            favs = favs.toList(),
            cached = cached.map { FavStation.of(it) },
            blocked = blocked.toList(),
            excluded = excluded.toList(),
        ),
    )

    /**
     * returns null for anything that is not one of our backups. the marker and
     * version are checked because restoring a stranger's json would overwrite the
     * user's real state with empty values.
     */
    fun decode(text: String): Backup? {
        val payload = runCatching {
            json.decodeFromString(BackupPayload.serializer(), text)
        }.getOrNull() ?: return null
        if (payload.app != MARKER || payload.version != VERSION) {
            return null
        }
        return Backup(
            key = payload.key,
            favs = payload.favs.toSet(),
            cached = payload.cached.map { it.toStation() },
            blocked = payload.blocked.toSet(),
            excluded = payload.excluded.toSet(),
        )
    }
}
