package net.vchub.r4dio

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.core.stringSetPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import kotlin.random.Random

enum class Scope { ALL, FAVS }

object FavLogic {
    fun toggle(favs: Set<String>, uuid: String): Set<String> =
        when (favs.contains(uuid)) {
            true -> favs - uuid
            false -> favs + uuid
        }

    fun pickFav(cached: List<Station>, rng: Random = Random.Default): Station? {
        val playable = cached.filter { allowedStation(it) }
        if (playable.isEmpty()) return null
        return playable[rng.nextInt(playable.size)]
    }
}

object SyncMerge {
    fun mergedFavs(local: Set<String>, remote: List<String>): Set<String> = local + remote
}

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore("r4dio")

class FavStore(context: Context) {
    private val store = context.applicationContext.dataStore
    private val json = Json { ignoreUnknownKeys = true }

    private val keyFavs = stringSetPreferencesKey("fav_uuids")
    private val keyScope = stringPreferencesKey("scope")
    private val keyCached = stringPreferencesKey("cached_favs")
    private val keySyncKey = stringPreferencesKey("sync_key")
    private val keyBlocked = stringSetPreferencesKey("blocked_uuids")
    private val keyDeviceId = stringPreferencesKey("device_id")

    val favUuids: Flow<Set<String>> = store.data.map { it[keyFavs] ?: emptySet() }

    val scope: Flow<Scope> = store.data.map {
        when (it[keyScope]) {
            Scope.FAVS.name -> Scope.FAVS
            else -> Scope.ALL
        }
    }

    val cachedFavs: Flow<List<Station>> = store.data.map { prefs ->
        val raw = prefs[keyCached] ?: return@map emptyList()
        runCatching {
            json.decodeFromString(ListSerializer(FavStation.serializer()), raw).map { it.toStation() }
        }.getOrDefault(emptyList())
    }

    suspend fun toggleFav(station: Station) {
        store.edit { prefs ->
            val current = prefs[keyFavs] ?: emptySet()
            val next = FavLogic.toggle(current, station.uuid)
            prefs[keyFavs] = next
            val cachedRaw = prefs[keyCached]
            val cached = when (cachedRaw) {
                null -> emptyList()
                else -> runCatching {
                    json.decodeFromString(ListSerializer(FavStation.serializer()), cachedRaw)
                }.getOrDefault(emptyList())
            }
            val nextCached = when (next.contains(station.uuid)) {
                true -> cached.filter { it.uuid != station.uuid } + FavStation.of(station)
                false -> cached.filter { it.uuid != station.uuid }
            }
            prefs[keyCached] = json.encodeToString(ListSerializer(FavStation.serializer()), nextCached)
        }
    }

    suspend fun setScope(scope: Scope) {
        store.edit { it[keyScope] = scope.name }
    }

    suspend fun currentFavUuids(): Set<String> = favUuids.first()
    suspend fun currentScope(): Scope = scope.first()
    suspend fun currentCachedFavs(): List<Station> = cachedFavs.first()

    suspend fun syncKey(): String? = store.data.first()[keySyncKey]

    suspend fun setSyncKey(key: String?) {
        store.edit { prefs ->
            when (key) {
                null -> prefs.remove(keySyncKey)
                else -> prefs[keySyncKey] = key
            }
        }
    }

    suspend fun currentBlocked(): Set<String> = store.data.first()[keyBlocked] ?: emptySet()

    suspend fun deviceId(): String {
        val existing = store.data.first()[keyDeviceId]
        when (existing) {
            null -> {}
            else -> return existing
        }
        val id = "dev-%08x".format(kotlin.random.Random.nextInt())
        store.edit { it[keyDeviceId] = id }
        return id
    }

    suspend fun applyMerged(favs: Set<String>, blocked: Set<String>) {
        store.edit { prefs ->
            prefs[keyFavs] = favs
            prefs[keyBlocked] = blocked
        }
    }
}
