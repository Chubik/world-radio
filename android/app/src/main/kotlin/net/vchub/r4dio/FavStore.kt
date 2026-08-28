package net.vchub.r4dio

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.MutablePreferences
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
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

    /**
     * user-excluded countries are deliberately not passed: an explicit favourite
     * outranks a country filter. [blocked] is passed, and that difference is the
     * point — blocking a station means never play it, star or no star.
     */
    fun pickFav(
        cached: List<Station>,
        blocked: Set<String> = emptySet(),
        rng: Random = Random.Default,
    ): Station? {
        val playable = cached.filter { allowedStation(it, blocked = blocked) }
        if (playable.isEmpty()) return null
        return playable[rng.nextInt(playable.size)]
    }
}

/**
 * the catalogue cache is filtered by excluded-country at fetch time (see
 * Catalog.fetchStations), so a change to that set makes the cache stop reflecting
 * the user's intent. pulled out as a pure function so the "only on an actual change"
 * rule is testable without a DataStore harness.
 */
object ExcludedCountries {
    fun normalize(countries: Set<String>): Set<String> = countries.map { it.uppercase() }.toSet()

    fun changed(previous: Set<String>, next: Set<String>): Boolean =
        normalize(previous) != normalize(next)
}

object SyncMerge {
    fun mergedFavs(local: Set<String>, remote: List<String>): Set<String> = local + remote

    private fun unionIds(local: List<String>, server: List<String>): List<String> =
        local + server.filterNot { local.contains(it) }

    fun mergedData(local: SyncData, server: SyncData): SyncData =
        SyncData(
            favs = unionIds(local.favs, server.favs),
            blocked = unionIds(local.blocked, server.blocked),
            excluded_countries = unionIds(local.excluded_countries, server.excluded_countries),
        )
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
    private val keyHiddenDead = stringSetPreferencesKey("hidden_dead")
    private val keyExcludedCountries = stringSetPreferencesKey("excluded_countries")
    private val keyDeviceId = stringPreferencesKey("device_id")
    private val keyCatalogSyncedAt = longPreferencesKey("catalog_synced_at")
    private val keyCatalogEtag = stringPreferencesKey("catalog_etag")
    private val keyKeepAwake = booleanPreferencesKey("keep_awake")
    private val keyFillOnMobile = booleanPreferencesKey("fill_on_mobile")
    private val keyFilterCountries = stringSetPreferencesKey("filter_countries")
    private val keyFilterAt = longPreferencesKey("filter_countries_at")
    // the scope as it travels, kept beside the local enum: the desktop has scopes
    // android cannot show, and storing only the enum would lose them and re-publish
    // an approximation that drags the desktop off the user's chosen scope.
    private val keyScopeWire = stringPreferencesKey("scope_wire")
    private val keyScopeAt = longPreferencesKey("scope_at")
    private val keyTheme = stringPreferencesKey("theme")
    private val keyThemeAt = longPreferencesKey("theme_at")
    private val keyHistoryPending = stringSetPreferencesKey("history_pending")

    // what this device changed since its last successful sync — see PendingChanges.
    // a plain listing of what we still hold cannot express a removal.
    private val keyPendingChanges = stringPreferencesKey("pending_changes")

    // the local play history, which is NOT keyHistoryPending: that one is a push
    // queue emptied by sync. see PlayHistory.
    private val keyPlayHistory = stringPreferencesKey("play_history")

    val favUuids: Flow<Set<String>> = store.data.map { it[keyFavs] ?: emptySet() }

    val blockedUuids: Flow<Set<String>> = store.data.map { it[keyBlocked] ?: emptySet() }

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
            val cached = decodeCached(prefs[keyCached])
            val nextCached = when (next.contains(station.uuid)) {
                true -> cached.filter { it.uuid != station.uuid } + FavStation.of(station)
                false -> cached.filter { it.uuid != station.uuid }
            }
            prefs[keyCached] = json.encodeToString(ListSerializer(FavStation.serializer()), nextCached)
            notePending(prefs, ChangeSet.FAVS, station.uuid, gone = !next.contains(station.uuid))
        }
    }

    /**
     * the local half of blocking. until now a uuid could only enter this set from
     * a sync merge or a backup restore, which meant a station could be blocked on
     * the desktop but not on the phone that is playing it.
     *
     * deliberately does not touch the favourite set: blocked outranks a star, and
     * allowedStation already resolves that — unstarring here would lose a choice
     * the user made separately.
     */
    suspend fun toggleBlocked(uuid: String) {
        store.edit { prefs ->
            val current = prefs[keyBlocked] ?: emptySet()
            val next = FavLogic.toggle(current, uuid)
            prefs[keyBlocked] = next
            notePending(prefs, ChangeSet.BLOCKED, uuid, gone = !next.contains(uuid))
        }
    }

    private fun decodeCached(raw: String?): List<FavStation> = when (raw) {
        null -> emptyList()
        else -> runCatching {
            json.decodeFromString(ListSerializer(FavStation.serializer()), raw)
        }.getOrDefault(emptyList())
    }

    private fun decodePending(raw: String?): PendingChanges = when (raw) {
        null -> PendingChanges()
        else -> runCatching {
            json.decodeFromString(PendingChanges.serializer(), raw)
        }.getOrDefault(PendingChanges())
    }

    // records into the same transaction as the mutation that calls it — a crash
    // between two separate transactions would leave the set and the pending list
    // disagreeing, and that disagreement is silent.
    private fun notePending(prefs: MutablePreferences, set: ChangeSet, id: String, gone: Boolean) {
        val next = decodePending(prefs[keyPendingChanges]).note(set, id, gone)
        prefs[keyPendingChanges] = json.encodeToString(PendingChanges.serializer(), next)
    }

    suspend fun currentPending(): PendingChanges = decodePending(store.data.first()[keyPendingChanges])

    suspend fun clearPushedPending(pushed: PendingChanges) {
        store.edit { prefs ->
            val next = decodePending(prefs[keyPendingChanges]).clearPushed(pushed)
            prefs[keyPendingChanges] = json.encodeToString(PendingChanges.serializer(), next)
        }
    }

    /**
     * rebuilds the cached station objects from resolved stations. sync overwrites the
     * uuid set without touching the objects, so the cache has to be rebuilt from it
     * rather than edited one star at a time.
     *
     * the uuid set is re-read inside the transaction, not passed in: resolving takes a
     * network round-trip, and a star tapped during it would otherwise be overwritten by
     * a caller holding a pre-tap snapshot — the same uuid/object divergence this whole
     * path exists to remove. the blocked set is re-read for the same reason, and applied
     * here so a station blocked after it was cached is evicted rather than kept alive by
     * the cached copy — the star itself stays, so unblocking restores it.
     */
    suspend fun reconcileCachedFavs(resolved: List<Station>) {
        store.edit { prefs ->
            val wanted = (prefs[keyFavs] ?: emptySet()) - (prefs[keyBlocked] ?: emptySet())
            val byUuid = LinkedHashMap<String, FavStation>()
            decodeCached(prefs[keyCached]).forEach { byUuid[it.uuid] = it }
            resolved.forEach { byUuid[it.uuid] = FavStation.of(it) }
            val next = wanted.mapNotNull { byUuid[it] }.filter { it.url.isNotBlank() }
            prefs[keyCached] = json.encodeToString(ListSerializer(FavStation.serializer()), next)
        }
    }

    /**
     * the stamp is taken here, at the moment the user changes the scope, never at
     * sync time — a sync-time stamp would always outrank the other device. [now] is
     * passed in so the caller's clock is the one recorded and the rule stays testable.
     */
    suspend fun setScope(scope: Scope, now: Long = System.currentTimeMillis() / 1000) {
        store.edit { prefs ->
            // through withScope so the "a same-value save must not move the stamp"
            // rule lives in exactly one place rather than being restated here.
            val stamped = SyncProfile(
                scope = prefs[keyScopeWire].orEmpty(),
                scopeAt = prefs[keyScopeAt] ?: 0L,
            ).withScope(wireScope(scope), now)
            prefs[keyScopeWire] = stamped.scope
            prefs[keyScopeAt] = stamped.scopeAt
            prefs[keyScope] = scope.name
        }
    }

    suspend fun currentFilter(): Set<String> = store.data.first()[keyFilterCountries] ?: emptySet()

    /**
     * the filter is shared across devices, so a change here travels like the scope
     * does. stamped through [SyncProfile.withCountries] so the "a same-value save
     * must not move the stamp" rule lives in one place.
     */
    suspend fun setFilter(countries: Set<String>, now: Long = System.currentTimeMillis() / 1000) {
        store.edit { prefs ->
            val stamped = SyncProfile(
                countries = (prefs[keyFilterCountries] ?: emptySet()).sorted(),
                countriesAt = prefs[keyFilterAt] ?: 0L,
            ).withCountries(countries.toList(), now)
            prefs[keyFilterCountries] = stamped.countries.toSet()
            prefs[keyFilterAt] = stamped.countriesAt
        }
    }

    val theme: Flow<String> = store.data.map { it[keyTheme].orEmpty() }

    suspend fun currentTheme(): String = store.data.first()[keyTheme].orEmpty()

    /**
     * stamped through [SyncProfile.withTheme] so the "a same-value save must not
     * move the stamp" rule lives in one place, exactly as setScope does.
     */
    suspend fun setTheme(slug: String, now: Long = System.currentTimeMillis() / 1000) {
        store.edit { prefs ->
            val stamped = SyncProfile(
                theme = prefs[keyTheme].orEmpty(),
                themeAt = prefs[keyThemeAt] ?: 0L,
            ).withTheme(slug, now)
            prefs[keyTheme] = stamped.theme
            prefs[keyThemeAt] = stamped.themeAt
        }
    }

    suspend fun profile(): SyncProfile {
        val prefs = store.data.first()
        return SyncProfile(
            // a set has no order, so the pill and the payload both need one that
            // does not shuffle between reads.
            countries = (prefs[keyFilterCountries] ?: emptySet()).sorted(),
            countriesAt = prefs[keyFilterAt] ?: 0L,
            scope = prefs[keyScopeWire].orEmpty(),
            scopeAt = prefs[keyScopeAt] ?: 0L,
            theme = prefs[keyTheme].orEmpty(),
            themeAt = prefs[keyThemeAt] ?: 0L,
        )
    }

    /**
     * writes back what a sync round-trip took from the server. the local [keyScope]
     * enum only moves when the synced word is one this build can show — a `recent`
     * or `dead` from the desktop is stored and re-published verbatim, but must not
     * approximate itself into ALL here.
     *
     * the stored stamps are re-read inside the transaction and run through
     * [SyncProfile.keepingNewerLocal] rather than trusted from the caller's
     * snapshot: [profile] was computed from a read taken before the round-trip, so
     * a scope tapped while the request was in flight would otherwise be reverted.
     */
    suspend fun applyProfile(profile: SyncProfile) {
        store.edit { prefs ->
            val stored = SyncProfile(
                countries = (prefs[keyFilterCountries] ?: emptySet()).sorted(),
                countriesAt = prefs[keyFilterAt] ?: 0L,
                scope = prefs[keyScopeWire].orEmpty(),
                scopeAt = prefs[keyScopeAt] ?: 0L,
                theme = prefs[keyTheme].orEmpty(),
                themeAt = prefs[keyThemeAt] ?: 0L,
            )
            val next = profile.keepingNewerLocal(stored)
            prefs[keyFilterCountries] = next.countries.toSet()
            prefs[keyFilterAt] = next.countriesAt
            prefs[keyScopeWire] = next.scope
            prefs[keyScopeAt] = next.scopeAt
            prefs[keyTheme] = next.theme
            prefs[keyThemeAt] = next.themeAt
            when (val local = localScope(next.scope)) {
                null -> {}
                else -> prefs[keyScope] = local.name
            }
        }
    }

    suspend fun pendingPlays(): Set<String> = store.data.first()[keyHistoryPending] ?: emptySet()

    suspend fun recordPlay(uuid: String, now: Long = System.currentTimeMillis() / 1000) {
        store.edit { prefs ->
            prefs[keyHistoryPending] = HistoryQueue.append(prefs[keyHistoryPending] ?: emptySet(), uuid, now)
        }
    }

    /**
     * what this phone has played, newest first. survives sync, needs no account,
     * and carries whole stations so a row renders and replays offline — none of
     * which the push queue behind [recordPlay] can do.
     */
    val playHistory: Flow<List<Station>> = store.data.map { prefs ->
        PlayHistory.stations(decodeHistory(prefs[keyPlayHistory]))
    }

    /**
     * takes the whole station, not a uuid: the caller has it, and looking it up
     * later would fail for exactly the stations that have since left the
     * catalogue — the ones a listener is most likely to be hunting for.
     */
    suspend fun recordPlayed(station: Station, now: Long = System.currentTimeMillis() / 1000) {
        store.edit { prefs ->
            val next = PlayHistory.append(decodeHistory(prefs[keyPlayHistory]), station, now)
            prefs[keyPlayHistory] =
                json.encodeToString(ListSerializer(HistoryEntry.serializer()), next)
        }
    }

    suspend fun clearPlayHistory() {
        store.edit { it.remove(keyPlayHistory) }
    }

    private fun decodeHistory(raw: String?): List<HistoryEntry> {
        if (raw == null) return emptyList()
        // unreadable history is not worth crashing over, and an empty list is
        // exactly what an untouched install shows anyway.
        return runCatching {
            json.decodeFromString(ListSerializer(HistoryEntry.serializer()), raw)
        }.getOrDefault(emptyList())
    }

    suspend fun drainPlays(pushed: List<HistoryRecord>) {
        store.edit { prefs ->
            prefs[keyHistoryPending] = HistoryQueue.drain(prefs[keyHistoryPending] ?: emptySet(), pushed)
        }
    }

    val keepAwake: Flow<Boolean> = store.data.map { it[keyKeepAwake] ?: false }

    suspend fun currentKeepAwake(): Boolean = keepAwake.first()

    suspend fun setKeepAwake(on: Boolean) {
        store.edit { it[keyKeepAwake] = on }
    }

    /**
     * whether the catalogue may be fetched over mobile data. on by default:
     * the whole catalogue is one 4.3 mb request, and a phone that never meets
     * wi-fi would otherwise sit on the skewed top-1000 forever. android's own
     * data saver still outranks this — see catalogueFetchAllowed.
     */
    val fillOnMobile: Flow<Boolean> = store.data.map { it[keyFillOnMobile] ?: true }

    suspend fun currentFillOnMobile(): Boolean = fillOnMobile.first()

    suspend fun setFillOnMobile(on: Boolean) {
        store.edit { it[keyFillOnMobile] = on }
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

    // stream health is local by design: a station dead on this device's network
    // may be fine elsewhere, so hidden_dead is never synced and never backed up.
    suspend fun currentHiddenDead(): Set<String> = store.data.first()[keyHiddenDead] ?: emptySet()

    suspend fun hideDead(uuid: String) {
        store.edit { prefs ->
            prefs[keyHiddenDead] = (prefs[keyHiddenDead] ?: emptySet()) + uuid
        }
    }

    suspend fun unhideDead(uuid: String) {
        store.edit { prefs ->
            prefs[keyHiddenDead] = (prefs[keyHiddenDead] ?: emptySet()) - uuid
        }
    }

    suspend fun pruneHiddenDead(keep: Set<String>) {
        store.edit { prefs ->
            prefs[keyHiddenDead] = pruneHidden(prefs[keyHiddenDead] ?: emptySet(), keep)
        }
    }

    val excludedCountries: Flow<Set<String>> =
        store.data.map { it[keyExcludedCountries] ?: emptySet() }

    suspend fun currentExcluded(): Set<String> =
        store.data.first()[keyExcludedCountries] ?: emptySet()

    /**
     * returns true when the excluded set actually changed, so the caller can decide
     * whether the catalogue cache — filtered at fetch time under the old set — needs
     * invalidating. a same-value save (dialog opened and saved untouched) must not
     * trigger a refetch.
     */
    suspend fun setExcluded(countries: Set<String>): Boolean {
        val next = ExcludedCountries.normalize(countries)
        var changed = false
        store.edit { prefs ->
            val prev = prefs[keyExcludedCountries] ?: emptySet()
            changed = ExcludedCountries.changed(prev, next)
            prefs[keyExcludedCountries] = next
            // the cache was fetched and filtered under the old set, so it no longer
            // reflects the user's intent — reset in the same transaction as the
            // exclusion write so no reader can observe the new filters with a stamp
            // that still claims the catalogue is fresh.
            when (changed) {
                true -> prefs[keyCatalogSyncedAt] = 0L
                false -> {}
            }
            (next - prev).forEach { notePending(prefs, ChangeSet.COUNTRIES, it, gone = false) }
            (prev - next).forEach { notePending(prefs, ChangeSet.COUNTRIES, it, gone = true) }
        }
        return changed
    }

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

    suspend fun catalogSyncedAt(): Long = store.data.first()[keyCatalogSyncedAt] ?: 0L

    suspend fun setCatalogSyncedAt(epochSecs: Long) {
        store.edit { it[keyCatalogSyncedAt] = epochSecs }
    }

    suspend fun currentCatalogEtag(): String = store.data.first()[keyCatalogEtag].orEmpty()

    suspend fun setCatalogEtag(etag: String) {
        store.edit { it[keyCatalogEtag] = etag }
    }

    /** same change-detection as [setExcluded] — a merge from another device can also
     *  shift the excluded set (union merge), which needs the same cache invalidation. */
    suspend fun applyMerged(favs: Set<String>, blocked: Set<String>, excluded: Set<String>): Boolean {
        val next = ExcludedCountries.normalize(excluded)
        var changed = false
        store.edit { prefs ->
            val prev = prefs[keyExcludedCountries] ?: emptySet()
            changed = ExcludedCountries.changed(prev, next)
            prefs[keyFavs] = favs
            prefs[keyBlocked] = blocked
            prefs[keyExcludedCountries] = next
            when (changed) {
                true -> prefs[keyCatalogSyncedAt] = 0L
                false -> {}
            }
        }
        return changed
    }

    /**
     * restores a file backup. one transaction rather than four separate writes, so
     * a restore interrupted halfway cannot leave the key pointing at one device's
     * account while the favourites belong to another.
     */
    suspend fun restore(backup: Backup) {
        val nextExcluded = ExcludedCountries.normalize(backup.excluded)
        val encoded = json.encodeToString(
            ListSerializer(FavStation.serializer()),
            backup.cached.map { FavStation.of(it) },
        )
        store.edit { prefs ->
            when (backup.key) {
                null -> prefs.remove(keySyncKey)
                else -> prefs[keySyncKey] = backup.key
            }
            prefs[keyFavs] = backup.favs
            prefs[keyBlocked] = backup.blocked
            prefs[keyExcludedCountries] = nextExcluded
            prefs[keyCached] = encoded
            // the catalogue was fetched under whatever filters this device had, so
            // it no longer reflects the restored ones.
            prefs[keyCatalogSyncedAt] = 0L
        }
    }
}
