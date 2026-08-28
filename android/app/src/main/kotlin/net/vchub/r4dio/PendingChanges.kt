package net.vchub.r4dio

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

enum class ChangeSet { FAVS, BLOCKED, COUNTRIES }

@Serializable
data class Change(val id: String, val gone: Boolean)

/**
 * what this device changed since its last successful sync. the server needs it
 * because a plain list of what we still hold cannot express a removal.
 */
@Serializable
data class PendingChanges(
    val favs: List<Change> = emptyList(),
    val blocked: List<Change> = emptyList(),
    // wire format name, kept snake_case to match the server's ChangeSets struct
    @SerialName("excluded_countries")
    val excluded_countries: List<Change> = emptyList(),
) {
    /** returns a copy with `id`'s change replacing any earlier one for the same id in `set`. */
    fun note(set: ChangeSet, id: String, gone: Boolean): PendingChanges {
        val change = Change(id, gone)
        return when (set) {
            ChangeSet.FAVS -> copy(favs = favs.filterNot { it.id == id } + change)
            ChangeSet.BLOCKED -> copy(blocked = blocked.filterNot { it.id == id } + change)
            ChangeSet.COUNTRIES -> copy(excluded_countries = excluded_countries.filterNot { it.id == id } + change)
        }
    }

    fun isEmpty(): Boolean = favs.isEmpty() && blocked.isEmpty() && excluded_countries.isEmpty()

    /**
     * post-sync clear for the case where `pushed` is what actually went out on the wire:
     * removes only the exact (id, gone) entries that were pushed, keeping anything written
     * meanwhile. an id whose `gone` flipped since the push is a new, unsent action and
     * must survive the clear.
     */
    fun clearPushed(pushed: PendingChanges): PendingChanges = PendingChanges(
        favs = favs.filterNot { it in pushed.favs },
        blocked = blocked.filterNot { it in pushed.blocked },
        excluded_countries = excluded_countries.filterNot { it in pushed.excluded_countries },
    )
}
