package net.vchub.r4dio

import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.CommandButton
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import kotlin.concurrent.thread
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout

const val CMD_SHUFFLE = "net.vchub.r4dio.SHUFFLE"
const val CMD_TOGGLE = "net.vchub.r4dio.TOGGLE"
const val CMD_STAR = "net.vchub.r4dio.STAR"
const val CMD_SCOPE = "net.vchub.r4dio.SCOPE"
const val CMD_STOP = "net.vchub.r4dio.STOP"
const val CMD_SYNC_UI = "net.vchub.r4dio.SYNC_UI"
const val CMD_CLEAR_FILTER = "net.vchub.r4dio.CLEAR_FILTER"
const val CMD_PLAY_UUID = "net.vchub.r4dio.PLAY_UUID"
const val ARG_UUID = "uuid"
const val ACTION_SYNC_NOW = "net.vchub.r4dio.SYNC_NOW"
const val EXTRA_FAV = "net.vchub.r4dio.EXTRA_FAV"
const val EXTRA_SCOPE = "net.vchub.r4dio.EXTRA_SCOPE"
const val EXTRA_FAV_COUNT = "net.vchub.r4dio.EXTRA_FAV_COUNT"
const val EXTRA_HIDDEN_COUNT = "net.vchub.r4dio.EXTRA_HIDDEN_COUNT"
const val EXTRA_PLAYABLE_COUNT = "net.vchub.r4dio.EXTRA_PLAYABLE_COUNT"
const val EXTRA_CATALOG_LOADED = "net.vchub.r4dio.EXTRA_CATALOG_LOADED"
const val EXTRA_FILTER_COUNTRIES = "net.vchub.r4dio.EXTRA_FILTER_COUNTRIES"
const val EXTRA_CATALOG_SIZE = "net.vchub.r4dio.EXTRA_CATALOG_SIZE"
const val EXTRA_CATALOG_GROWING = "net.vchub.r4dio.EXTRA_CATALOG_GROWING"

/** true only while a catalogue download is actually in flight. */
const val EXTRA_CATALOG_FETCHING = "net.vchub.r4dio.EXTRA_CATALOG_FETCHING"

private class ShufflePlayer(
    delegate: androidx.media3.common.Player,
    private val onShuffle: () -> Unit,
) : androidx.media3.common.ForwardingPlayer(delegate) {

    private val extraCommands = intArrayOf(
        androidx.media3.common.Player.COMMAND_SEEK_TO_NEXT,
        androidx.media3.common.Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
        androidx.media3.common.Player.COMMAND_SEEK_TO_PREVIOUS,
        androidx.media3.common.Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
    )

    override fun getAvailableCommands(): androidx.media3.common.Player.Commands {
        val builder = super.getAvailableCommands().buildUpon()
        extraCommands.forEach { builder.add(it) }
        return builder.build()
    }

    override fun isCommandAvailable(command: Int): Boolean =
        command in extraCommands || super.isCommandAvailable(command)

    override fun seekToNext() = onShuffle()
    override fun seekToNextMediaItem() = onShuffle()
    override fun seekToPrevious() = onShuffle()
    override fun seekToPreviousMediaItem() = onShuffle()
}

class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null
    private var exo: ExoPlayer? = null
    private val catalog = Catalog()
    private val catalogCache by lazy { CatalogCache(filesDir) }
    private val toast by lazy { StationToast(this) }
    @Volatile private var stations: List<Station> = emptyList()
    // true once a load has been attempted and resolved, on either branch — including
    // the branch where the user's filters emptied the fetch and `stations` stays
    // empty. distinct from `stations.isNotEmpty()`, which would wrongly read as
    // "not loaded" in that case and permanently suppress the all-hidden warn.
    @Volatile private var catalogAttempted = false
    // collapses concurrent refreshIfStale() callers (cold-start loadStations() and
    // syncNow(), which both can run in the first seconds of a service) into a single
    // network fetch instead of one each.
    private val refreshInFlight = java.util.concurrent.atomic.AtomicBoolean(false)
    // same collapsing as refreshInFlight: a sync burst and service start can all
    // reach pullFilteredCountries() within a second of each other.
    private val filterPullInFlight = java.util.concurrent.atomic.AtomicBoolean(false)
    // countries already pulled in full this session, so a re-sync that carries the
    // same filter costs nothing. deliberately not persisted: a fresh process is
    // also a fresh chance to pick up stations added since.
    @Volatile private var filterCountriesPulled: Set<String> = emptySet()
    private val topUpInFlight = java.util.concurrent.atomic.AtomicBoolean(false)
    private val conditions by lazy { TopUpConditions(this) }
    @Volatile private var current: Station? = null
    private val health = HealthTracker()
    @Volatile private var mirrorSeq: Long = 0
    @Volatile private var applyingMirror: Boolean = false
    // the debounce: set while a doorbell-triggered sync is queued, cleared once
    // it has run. a burst of events costs one re-sync, not one per event.
    private val resyncQueued = java.util.concurrent.atomic.AtomicBoolean(false)
    private val artwork: ByteArray by lazy { crtArtworkPng() }
    private var mirrorJob: Job? = null
    private val main = Handler(Looper.getMainLooper())
    private val favStore by lazy { FavStore(this) }
    private val syncClient = SyncClient()
    private val mirrorClient = MirrorClient()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    private val shuffleCommand = SessionCommand(CMD_SHUFFLE, android.os.Bundle.EMPTY)
    private val toggleCommand = SessionCommand(CMD_TOGGLE, android.os.Bundle.EMPTY)
    private val starCommand = SessionCommand(CMD_STAR, android.os.Bundle.EMPTY)
    private val scopeCommand = SessionCommand(CMD_SCOPE, android.os.Bundle.EMPTY)
    private val stopCommand = SessionCommand(CMD_STOP, android.os.Bundle.EMPTY)
    private val syncUiCommand = SessionCommand(CMD_SYNC_UI, android.os.Bundle.EMPTY)
    private val clearFilterCommand = SessionCommand(CMD_CLEAR_FILTER, android.os.Bundle.EMPTY)
    private val playUuidCommand = SessionCommand(CMD_PLAY_UUID, android.os.Bundle.EMPTY)

    private val shuffleButton = CommandButton.Builder(CommandButton.ICON_SHUFFLE_ON)
        .setDisplayName("shuffle")
        .setCustomIconResId(R.drawable.ic_shuffle)
        .setSessionCommand(shuffleCommand)
        .build()

    private val stopButton = CommandButton.Builder(CommandButton.ICON_STOP)
        .setDisplayName("stop")
        .setSessionCommand(stopCommand)
        .build()

    // no sync button in the notification: the slots are scarce (3 visible when
    // collapsed) and sync lives on the home screen, which has room for it.
    // CMD_SYNC_UI stays available to controllers, it just isn't shown here.

    private fun starButton(isFav: Boolean) = CommandButton.Builder(
        if (isFav) CommandButton.ICON_STAR_FILLED else CommandButton.ICON_STAR_UNFILLED,
    )
        .setDisplayName("favs")
        .setCustomIconResId(if (isFav) R.drawable.ic_star else R.drawable.ic_star_outline)
        .setSessionCommand(starCommand)
        .build()

    private fun scopeButton(scope: Scope) = CommandButton.Builder(CommandButton.ICON_UNDEFINED)
        .setDisplayName(if (scope == Scope.FAVS) "favs only" else "all")
        .setCustomIconResId(if (scope == Scope.FAVS) R.drawable.ic_scope_favs else R.drawable.ic_scope_all)
        .setSessionCommand(scopeCommand)
        .build()

    private suspend fun refreshCustomLayout() {
        val favs = favStore.currentFavUuids()
        val isFav = current?.uuid?.let { favs.contains(it) } ?: false
        val sc = favStore.currentScope()
        val hidden = favStore.currentExcluded()
        val blocked = favStore.currentBlocked()
        val included = favStore.currentFilter()
        // count what the user could actually reach, so the screen can tell
        // "your filters hid everything" apart from "the catalogue is empty".
        // dispatched: this runs on nearly every user action and the catalogue is
        // ~59k stations, each checked against 13 banned substrings — on Main that
        // is a visible stall on every star, scope and shuffle.
        val playable = withContext(Dispatchers.Default) {
            stations.count { allowedStation(it, hidden, blocked, included) }
        }
        // loadStations() (a raw thread) and syncNow() (a Main coroutine) race with no
        // ordering, so playableCount can be 0 just because the catalogue has not
        // landed yet. this flag is attempted-ness, not station presence, never a
        // filtered-vs-unfiltered difference, so it tells the two cases apart safely.
        val catalogLoaded = catalogAttempted
        session?.setCustomLayout(listOf(starButton(isFav), shuffleButton, scopeButton(sc), stopButton))
        val extras = android.os.Bundle().apply {
            putBoolean(EXTRA_FAV, isFav)
            putString(EXTRA_SCOPE, if (sc == Scope.FAVS) "favs" else "all")
            putInt(EXTRA_FAV_COUNT, favs.size)
            putInt(EXTRA_HIDDEN_COUNT, hidden.size)
            putInt(EXTRA_PLAYABLE_COUNT, playable)
            putBoolean(EXTRA_CATALOG_LOADED, catalogLoaded)
            putStringArray(EXTRA_FILTER_COUNTRIES, included.sorted().toTypedArray())
            putInt(EXTRA_CATALOG_SIZE, stations.size)
            // "+" means "this is not the whole catalogue yet", not "a fetch is
            // running" — that is its own flag, published by publishFetching.
            // measured against CATALOGUE_WHOLE rather than the upstream ceiling:
            // our filtering means the full catalogue never reaches that number,
            // so the old comparison left "+" showing on a complete catalogue.
            putBoolean(EXTRA_CATALOG_GROWING, stations.isNotEmpty() && stations.size < CATALOGUE_WHOLE)
        }
        session?.setSessionExtras(extras)
    }

    // one place that knows what the widget needs, so the five call sites cannot drift.
    private fun refreshWidget(station: Station?, isPlaying: Boolean, favs: Set<String>) {
        RadioWidgetProvider.refresh(
            context = this,
            station = station?.name.orEmpty(),
            meta = station?.let { widgetMetaLabel(it.country, it.codec, it.bitrate) }.orEmpty(),
            isPlaying = isPlaying,
            isFav = station?.uuid?.let { favs.contains(it) } ?: false,
        )
    }

    // for the three call sites with no coroutine context in hand (ExoPlayer listener,
    // playPick, the mirror-event branch). runBlocking does not deadlock here: it
    // installs its own event loop, so the DataStore read resumes on the blocked thread
    // rather than waiting on Main. keep this read cheap — DataStore's data flow has no
    // flowOn, so it runs on the caller's thread, and after the first read it is served
    // from memory cache.
    private fun refreshWidget(station: Station?, isPlaying: Boolean) =
        refreshWidget(station, isPlaying, runBlocking { favStore.currentFavUuids() })

    override fun onCreate() {
        super.onCreate()
        val player = ExoPlayer.Builder(this).build()
        player.addListener(object : androidx.media3.common.Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                if (isPlaying) {
                    health.onSuccess()
                    current?.let { st -> scope.launch { favStore.unhideDead(st.uuid) } }
                }
                refreshWidget(current, isPlaying)
            }

            override fun onPlayerError(error: androidx.media3.common.PlaybackException) {
                val blame = shouldBlame(error.errorCode)
                Log.w("r4dio", "playback error: ${error.errorCodeName}, blame=$blame, skipping station")
                if (health.onError(blame)) {
                    current?.let { st -> scope.launch { favStore.hideDead(st.uuid) } }
                }
                shuffle()
            }
        })
        exo = player
        val sessionPlayer = ShufflePlayer(player) { shuffle() }
        session = MediaSession.Builder(this, sessionPlayer)
            .setCallback(Callback())
            .build()
        val provider = androidx.media3.session.DefaultMediaNotificationProvider.Builder(this).build()
        provider.setSmallIcon(R.drawable.ic_stat_r4dio)
        setMediaNotificationProvider(provider)
        loadStations()
        syncNow()
        // the filter is usually already set, from a desktop that chose it long ago;
        // waiting for it to change would never pull anything for that user.
        pullFilteredCountries()
        // the baseline for someone who never sets a filter; it declines itself when
        // the phone is not on wi-fi and charging, so calling it costs nothing.
        topUpCatalogue()
        startMirrorListener()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_SYNC_NOW -> syncNow()
        }
        return super.onStartCommand(intent, flags, startId)
    }

    override fun onTaskRemoved(rootIntent: android.content.Intent?) {
        val player = exo
        val stillLoading = player != null && player.mediaItemCount == 0
        when (player != null && (player.playWhenReady || stillLoading)) {
            true -> {}
            false -> pauseAllPlayersAndStopSelf()
        }
    }

    override fun onDestroy() {
        // a panel left behind would outlive the thing it describes.
        toast.hide()
        session?.release()
        exo?.release()
        session = null
        exo = null
        mirrorJob?.cancel()
        scope.cancel()
        super.onDestroy()
    }

    private fun loadStations() {
        thread {
            val userExcluded = runBlocking { favStore.currentExcluded() }
            // the cache on disk was filtered under whatever blocked set existed when it
            // was written, so a station blocked since then is still in it — read the
            // current set and filter on every read path, not only at fetch time.
            val blocked = runBlocking { favStore.currentBlocked() }
            val hidden = runBlocking { favStore.currentHiddenDead() }
            val included = runBlocking { favStore.currentFilter() }
            val cached = catalogCache.read()
            when (cached.isEmpty()) {
                true -> {
                    val fetched = fetchAndStore(userExcluded, blocked)
                    catalogAttempted = true
                    when (fetched) {
                        // empty cache and an empty fetch: startFrom is never reached,
                        // so nothing else republishes the extras with the now-true
                        // catalogAttempted — without this the screen can stay on
                        // whatever syncNow() published first, permanently stale.
                        null -> scope.launch { refreshCustomLayout() }
                        else -> startFrom(fetched, userExcluded, blocked + hidden, included)
                    }
                }
                false -> {
                    stations = cached
                    Log.i("r4dio", "loaded ${cached.size} stations from cache")
                    // set before startFrom so its null branch's refresh — scheduled on
                    // Main, not run inline — is guaranteed to observe true, not a stale
                    // false read from before this attempt resolved.
                    catalogAttempted = true
                    startFrom(cached, userExcluded, blocked + hidden, included)
                    // the held catalogue predates genres; drop the stamp so the
                    // existing staleness path refetches it once, now.
                    if (catalogCache.needsGenreBackfill(cached)) {
                        Log.i("r4dio", "catalogue has no genres, refetching once")
                        runBlocking { favStore.setCatalogSyncedAt(0) }
                    }
                    refreshIfStale(userExcluded, blocked)
                }
            }
        }
    }

    private fun startFrom(
        list: List<Station>,
        userExcluded: Set<String>,
        blocked: Set<String>,
        included: Set<String>,
    ) {
        val pick = pickRandom(list, userExcluded, blocked, included = included)
        when (pick) {
            // catalogue loaded but the user's filters left nothing playable: the
            // screen is still on its initial idle state, so it needs the fresh
            // counts to show the warn instead of silently doing nothing.
            null -> scope.launch { refreshCustomLayout() }
            else -> main.post { playPick(pick) }
        }
    }

    /** returns the fetched list, or null when the network gave us nothing. */
    private fun fetchAndStore(userExcluded: Set<String>, blocked: Set<String>): List<Station>? {
        val fetched = catalog.fetchStations(userExcluded = userExcluded, blocked = blocked)
        if (fetched.isEmpty()) {
            Log.w("r4dio", "catalog fetch returned nothing")
            return null
        }
        stations = fetched
        // only stamp the sync time when the catalogue really landed on disk,
        // otherwise a fresh timestamp would suppress the retry we need.
        if (!catalogCache.write(fetched)) {
            Log.w("r4dio", "catalog cached in memory only, not stamping sync time")
            return fetched
        }
        runBlocking { favStore.setCatalogSyncedAt(nowSecs()) }
        runBlocking {
            favStore.pruneHiddenDead(
                fetched.map { it.uuid }.toSet() + favStore.currentFavUuids(),
            )
        }
        Log.i("r4dio", "fetched ${fetched.size} stations")
        return fetched
    }

    // a stale cache still plays; the refresh only replaces it if it succeeds.
    private fun refreshIfStale(userExcluded: Set<String>, blocked: Set<String>) {
        val syncedAt = runBlocking { favStore.catalogSyncedAt() }
        if (!catalogIsStale(syncedAt, nowSecs())) {
            return
        }
        // cold start can reach this from both loadStations() and syncNow() within the
        // same second; without this guard they would both pass the staleness check
        // and fire two network fetches for one actual staleness event.
        if (!refreshInFlight.compareAndSet(false, true)) {
            return
        }
        thread {
            try {
                val fetched = fetchAndStore(userExcluded, blocked)
                // a successful refetch can change playableCount — including 0 to
                // non-zero, which is exactly the hidden-countries recovery case — so
                // the screen needs to learn about it instead of waiting for the next
                // event that happens to call refreshCustomLayout() on its own.
                when (fetched) {
                    null -> {}
                    else -> scope.launch { refreshCustomLayout() }
                }
            } finally {
                refreshInFlight.set(false)
            }
        }
    }

    /**
     * pulls every station of each filtered country into the cache.
     *
     * called from all three places a filter can become active — the two syncs that
     * can write one, and service start, which is the case for a filter set on the
     * desktop days ago and never touched since. a change-only trigger would be dead
     * code for that user: their filter never changes again.
     */
    private fun pullFilteredCountries() {
        if (!filterPullInFlight.compareAndSet(false, true)) {
            return
        }
        thread {
            try {
                val filter = runBlocking { favStore.currentFilter() }
                val wanted = countriesToPull(filter, filterCountriesPulled)
                if (wanted.isEmpty()) {
                    return@thread
                }
                val blocked = runBlocking { favStore.currentBlocked() }
                var added = 0
                wanted.forEach { code ->
                    val fetched = catalog.fetchCountry(code, blocked = blocked)
                    // a failed fetch must not mark the country done, or it would
                    // never be retried for the rest of the session.
                    if (fetched.isEmpty()) {
                        Log.w("r4dio", "country fetch for $code returned nothing")
                        return@forEach
                    }
                    filterCountriesPulled = filterCountriesPulled + code
                    added += catalogCache.merge(fetched)
                }
                if (added == 0) {
                    return@thread
                }
                // the merged stations are only in the pool once the service re-reads
                // the cache: `stations` is what every pick path draws from.
                stations = catalogCache.read()
                Log.i("r4dio", "pulled $added new stations for filter ${wanted.joinToString(",")}")
                scope.launch { refreshCustomLayout() }
            } finally {
                filterPullInFlight.set(false)
            }
        }
    }

    /**
     * pages of the world catalogue, but only while the phone is on wi-fi and
     * charging. this is the baseline for the user who never sets a filter at all:
     * without it they stay on the skewed top-1000 forever.
     *
     * one opportunity now fetches pages until a condition fails, the ceiling is
     * reached, or a page adds nothing — unmetered/charging are re-read before
     * every single page, never cached, so unplugging stops the run within a page.
     */
    private fun topUpCatalogue() {
        if (!topUpInFlight.compareAndSet(false, true)) {
            return
        }
        thread {
            try {
                val held = catalogCache.read().size
                if (held >= TOP_UP_CEILING) {
                    return@thread
                }
                val allowed = catalogueFetchAllowed(
                    unmetered = conditions.unmetered(),
                    dataSaver = conditions.dataSaverOn(),
                    onMobileAllowed = runBlocking { favStore.currentFillOnMobile() },
                )
                if (!allowed) {
                    Log.i("r4dio", "catalogue fetch skipped: held=$held")
                    return@thread
                }
                val blocked = runBlocking { favStore.currentBlocked() }
                publishFetching(true)
                // one request for the whole catalogue, from our own server. it is
                // 4.3 mb where walking radio-browser for the same stations was 69 mb
                // over 312 requests, which is why this no longer waits for wi-fi.
                val fetched = catalog.fetchCatalogue(blocked = blocked)
                // empty means the download failed. writing it would erase a good
                // catalogue, so the old one stays and the next attempt retries.
                if (fetched.isEmpty()) {
                    Log.w("r4dio", "catalogue fetch returned nothing, keeping the ${held} held")
                    return@thread
                }
                // never trade a bigger catalogue for a smaller one: a partial
                // response should look like a failure, not like stations vanishing.
                if (fetched.size < held) {
                    Log.w("r4dio", "catalogue fetch returned ${fetched.size} against $held held, ignoring")
                    return@thread
                }
                if (!catalogCache.write(fetched)) {
                    Log.w("r4dio", "catalogue fetched but not stored, keeping it in memory only")
                    stations = fetched
                    return@thread
                }
                stations = fetched
                runBlocking { favStore.setCatalogSyncedAt(nowSecs()) }
                Log.i("r4dio", "catalogue fetched: ${fetched.size} stations in one request")
                scope.launch { refreshCustomLayout() }
            } finally {
                publishFetching(false)
                topUpInFlight.set(false)
            }
        }
    }

    /**
     * publishes only the in-flight flag, without the ~59k-station count and the
     * notification rebuild [refreshCustomLayout] does. that one is far too
     * expensive to run for a two-state boolean.
     *
     * every other field is left out of the bundle on purpose: uiStateFromExtras
     * folds onto the previous state, so an absent key keeps what was there.
     */
    private fun publishFetching(fetching: Boolean) {
        val extras = android.os.Bundle().apply {
            putBoolean(EXTRA_CATALOG_FETCHING, fetching)
        }
        main.post { session?.setSessionExtras(extras) }
    }

    private fun nowSecs(): Long = System.currentTimeMillis() / 1000

    private fun launchSyncActivity() {
        // android 16 blocks a background service from starting an activity even
        // with both BAL opt-ins, so we post a tappable notification instead —
        // launching from the user's tap on the notification is always allowed.
        val intent = android.content.Intent(this, SyncActivity::class.java)
            .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
        val flags = android.app.PendingIntent.FLAG_IMMUTABLE or
            android.app.PendingIntent.FLAG_UPDATE_CURRENT
        val contentIntent = android.app.PendingIntent.getActivity(this, 1, intent, flags)

        val channelId = "r4dio_sync"
        val nm = getSystemService(android.app.NotificationManager::class.java)
        nm.createNotificationChannel(
            android.app.NotificationChannel(
                channelId,
                "sync",
                android.app.NotificationManager.IMPORTANCE_HIGH,
            ),
        )
        val notif = android.app.Notification.Builder(this, channelId)
            .setSmallIcon(R.drawable.ic_sync)
            .setContentTitle("r4dio sync")
            .setContentText("tap to open sync settings")
            .setContentIntent(contentIntent)
            .setAutoCancel(true)
            .build()
        nm.notify(42, notif)
    }

    /**
     * sync replaces the favourite uuid set but knows nothing about station objects,
     * and playback picks from the cached objects — so without this a favourite
     * starred on another device can never play here. resolve from the catalogue
     * first (free), then fetch whatever is left (most favourites are outside the
     * top-1000 the catalogue holds).
     */
    private suspend fun reconcileFavCache() {
        val blocked = favStore.currentBlocked()
        // a blocked station stays starred — unblocking it on any device must bring the
        // star back — so it is dropped here rather than unstarred, and the fetch below
        // never spends a round-trip resolving something that cannot play.
        val wanted = favStore.currentFavUuids() - blocked
        // withReadyCatalog, not the bare field: syncNow races loadStations at startup, and
        // an empty catalogue would make every favourite look missing and be re-fetched.
        val known = favStore.currentCachedFavs() + withReadyCatalog()
        val missing = FavSync.missingUuids(wanted, known)
        val fetched = when (missing.isEmpty()) {
            true -> emptyList()
            else -> withContext(Dispatchers.IO) { catalog.fetchByUuids(missing, blocked) }
        }
        // hand the resolved stations over rather than the finished list: the store settles
        // them against the uuid set inside one transaction, so a star tapped during the
        // fetch above survives.
        favStore.reconcileCachedFavs(FavSync.reconcile(wanted, known, fetched))
        val resolved = favStore.currentCachedFavs().size
        Log.i("r4dio", "fav cache reconciled: $resolved/${wanted.size} resolved")
    }

    // the Job is what the doorbell debounce joins on, so the queued flag is not
    // cleared until this sync has actually finished.
    private fun syncNow(): Job {
        return scope.launch {
            val key = favStore.syncKey()
            when (key) {
                // no linked device: nothing to merge, but local settings (including the
                // excluded-country set) may have just changed via setExcluded(), which
                // is the common case since most installs have no linked device — so
                // still act on a stamp reset before refreshing the extras.
                null -> {
                    reconcileFavCache()
                    refreshIfStale(favStore.currentExcluded(), favStore.currentBlocked())
                    refreshCustomLayout()
                }
                else -> {
                    val profile = favStore.profile()
                    val plays = HistoryQueue.records(favStore.pendingPlays())
                    val local = profile.outgoing(
                        favs = favStore.currentFavUuids().toList(),
                        blocked = favStore.currentBlocked().toList(),
                        excluded = favStore.currentExcluded().toList(),
                        plays = plays,
                    )
                    val merged = withContext(Dispatchers.IO) { syncClient.push(key, local) } ?: return@launch
                    favStore.applyMerged(
                        merged.favs.toSet(),
                        merged.blocked.toSet(),
                        merged.excluded_countries.toSet(),
                    )
                    // drain exactly what was sent, so a station played during the
                    // round-trip stays queued for the next push.
                    favStore.drainPlays(plays)
                    val nextProfile = profile.applyRemote(merged)
                    when (nextProfile == profile) {
                        true -> {}
                        false -> favStore.applyProfile(nextProfile)
                    }
                    reconcileFavCache()
                    // setExcluded()/applyMerged() already reset the sync stamp when the
                    // excluded set actually changed — this is what acts on that reset
                    // within the running service, since refreshIfStale() is otherwise
                    // only reached once, from loadStations()'s cache-hit branch at
                    // startup. always safe to call: it is itself TTL-gated, so it is a
                    // no-op unless a reset (or real TTL expiry) actually happened.
                    refreshIfStale(favStore.currentExcluded(), favStore.currentBlocked())
                    // a filter that just arrived from another device names countries
                    // the top-1000 barely covers; pull them before the user shuffles.
                    pullFilteredCountries()
                    refreshCustomLayout()
                }
            }
        }
    }

    private fun mirrorAnnounce(pick: Station) {
        if (applyingMirror) {
            return
        }
        scope.launch {
            val key = favStore.syncKey() ?: return@launch
            val origin = favStore.deviceId()
            withContext(Dispatchers.IO) {
                mirrorClient.play(key, pick.uuid, pick.name, pick.url, origin)
            }
        }
    }

    private fun startMirrorListener() {
        mirrorJob = scope.launch(Dispatchers.IO) {
            while (isActive) {
                val key = favStore.syncKey()
                when (key) {
                    null -> delay(10_000)
                    else -> {
                        val myId = favStore.deviceId()
                        mirrorClient.events(key) { evt ->
                            when (runBlocking { favStore.syncKey() } == key) {
                                false -> {}
                                true -> onStreamEvent(evt, myId)
                            }
                        }
                        delay(3_000)
                    }
                }
            }
        }
    }

    /**
     * what the account event stream does to this service. a play mirrors the
     * other device; the doorbell queues one re-sync at a time. our own push
     * echoes back here too, and the re-sync it causes is a no-op the server
     * answers without ringing again, so it cannot loop.
     */
    private fun onStreamEvent(evt: StreamEvent, myId: String) {
        when (evt) {
            is StreamEvent.Play -> scope.launch { onMirrorEvent(evt.event, myId) }
            is StreamEvent.ProfileChanged -> {
                when (ResyncGate.claim(resyncQueued)) {
                    false -> {}
                    true -> scope.launch {
                        try {
                            // join, not just launch: the flag must stay set for
                            // the whole sync so events arriving while it is in
                            // flight collapse into this one.
                            syncNow().join()
                        } finally {
                            ResyncGate.release(resyncQueued)
                        }
                    }
                }
            }
        }
    }

    private fun onMirrorEvent(evt: MirrorEvent, myId: String) {
        when {
            evt.origin == myId -> return
            evt.seq <= mirrorSeq -> return
            else -> {}
        }
        mirrorSeq = evt.seq
        val station = Station(evt.uuid, evt.name, evt.url, "", "", 0)
        val userExcluded = runBlocking { favStore.currentExcluded() }
        val blocked = runBlocking { favStore.currentBlocked() }
        if (isExcluded(station) || station.country.uppercase() in userExcluded || station.uuid in blocked) {
            return
        }
        when (exo?.isPlaying) {
            true -> {
                applyingMirror = true
                playPick(station)
                applyingMirror = false
            }
            else -> {
                // announced even though nothing starts playing: the other device
                // changed station, and that is worth seeing. announced before
                // `current` moves, since the previous station is what decides
                // whether this is a change at all.
                announceStation(station)
                current = station
                // mirror events carry no country/codec/bitrate, so meta has nothing to
                // show — refreshWidget's widgetMetaLabel call resolves to "" on its own.
                refreshWidget(station, false)
            }
        }
    }

    private fun shuffle() {
        // a shuffle is the one moment we know the user is listening, so it is when
        // a page is worth fetching — still gated on wi-fi and charging.
        topUpCatalogue()
        scope.launch {
            val sc = withContext(Dispatchers.IO) {
                runCatching { withTimeout(3000) { favStore.currentScope() } }.getOrDefault(Scope.ALL)
            }
            val favs = withContext(Dispatchers.IO) {
                runCatching { withTimeout(3000) { favStore.currentCachedFavs() } }.getOrDefault(emptyList<Station>())
            }
            val userExcluded = withContext(Dispatchers.IO) {
                runCatching { withTimeout(3000) { favStore.currentExcluded() } }.getOrDefault(emptySet<String>())
            }
            val blocked = withContext(Dispatchers.IO) {
                runCatching { withTimeout(3000) { favStore.currentBlocked() } }.getOrDefault(emptySet<String>())
            }
            val hidden = withContext(Dispatchers.IO) {
                runCatching { withTimeout(3000) { favStore.currentHiddenDead() } }.getOrDefault(emptySet<String>())
            }
            val included = withContext(Dispatchers.IO) {
                runCatching { withTimeout(3000) { favStore.currentFilter() } }.getOrDefault(emptySet<String>())
            }
            val cat = withReadyCatalog()
            // shuffle is the gesture the steering-wheel key is bound to, and the
            // pick filters the whole ~59k catalogue before choosing. on Main that
            // stalls the tap that asked for it.
            val picked = withContext(Dispatchers.Default) {
                pickForScopeDetailed(sc, cat, favs, userExcluded, blocked + hidden, included = included)
            }
            if (picked.usedFallback) {
                Log.i("r4dio", "favs scope: no playable favourites, falling back to all stations")
            }
            when (val pick = picked.station) {
                // same case as startFrom's null branch: nothing playable for this
                // scope, and the user is looking at a screen that will not update
                // itself otherwise — refresh so the warn (if any) can show.
                null -> {
                    Log.i("r4dio", "shuffle: nothing to play for scope $sc")
                    refreshCustomLayout()
                }
                else -> playPick(pick)
            }
        }
    }

    private suspend fun withReadyCatalog(): List<Station> {
        val cur = stations
        if (cur.isNotEmpty()) return cur
        val cached = withContext(Dispatchers.IO) { catalogCache.read() }
        if (cached.isNotEmpty()) {
            stations = cached
            return cached
        }
        val userExcluded = withContext(Dispatchers.IO) { favStore.currentExcluded() }
        val blocked = withContext(Dispatchers.IO) { favStore.currentBlocked() }
        // delegate to fetchAndStore so there is one place that guards against an
        // empty fetch clobbering the cache, not two independently-maintained ones.
        val fetched = withContext(Dispatchers.IO) { fetchAndStore(userExcluded, blocked) }
        catalogAttempted = true
        return fetched ?: emptyList()
    }

    /** the station name over whatever is on screen, for the case the media
     *  notification cannot serve: a driver following a map never opens the
     *  shade. silent unless the user granted the overlay permission. */
    private fun announceStation(pick: Station) {
        if (!shouldAnnounce(current?.uuid, pick.uuid, StationToast.appIsInForeground)) {
            return
        }
        toast.show(toastText(pick.name, pick.country))
    }

    private fun playPick(pick: Station) {
        val player = exo ?: return
        // announced before `current` moves on, since the previous station is
        // what decides whether this is a change worth showing at all.
        announceStation(pick)
        current = pick
        refreshWidget(pick, true)
        Log.i("r4dio", "playing ${pick.name} — ${pick.url}")
        val subtitle = listOf(pick.country, pick.codec, "${pick.bitrate}k")
            .filter { it.isNotBlank() && it != "0k" }
            .joinToString(" · ")
        val metadata = MediaMetadata.Builder()
            .setTitle(pick.name)
            .setArtist(subtitle)
            .setStation(pick.name)
            .setIsBrowsable(false)
            .setIsPlayable(true)
            .setArtworkData(artwork, MediaMetadata.PICTURE_TYPE_FRONT_COVER)
            .build()
        val item = MediaItem.Builder()
            .setMediaId(pick.uuid)
            .setUri(pick.url)
            .setMediaMetadata(metadata)
            .build()
        val started = runCatching {
            player.setMediaItem(item)
            player.prepare()
            player.play()
        }
        when (started.isFailure) {
            true -> Log.w("r4dio", "cannot play ${pick.name}: ${started.exceptionOrNull()?.message}")
            false -> {
                // the stamp is the moment it played, taken here and never rewritten at
                // sync time — re-stamping would make every local entry outrank every
                // remote one and pin them all at the top of the cap.
                scope.launch {
                    favStore.recordPlay(pick.uuid, nowSecs())
                    refreshCustomLayout()
                }
                mirrorAnnounce(pick)
            }
        }
    }

    private inner class Callback : MediaSession.Callback {
        override fun onConnect(
            session: MediaSession,
            controller: MediaSession.ControllerInfo,
        ): MediaSession.ConnectionResult {
            val sessionCommands =
                MediaSession.ConnectionResult.DEFAULT_SESSION_AND_LIBRARY_COMMANDS.buildUpon()
                    .add(shuffleCommand)
                    .add(toggleCommand)
                    .add(starCommand)
                    .add(scopeCommand)
                    .add(stopCommand)
                    .add(syncUiCommand)
                    .add(clearFilterCommand)
                    .add(playUuidCommand)
                    .build()
            val playerCommands =
                MediaSession.ConnectionResult.DEFAULT_PLAYER_COMMANDS.buildUpon()
                    .build()
            return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
                .setAvailableSessionCommands(sessionCommands)
                .setAvailablePlayerCommands(playerCommands)
                .setCustomLayout(listOf(starButton(false), shuffleButton, scopeButton(Scope.ALL), stopButton))
                .build()
        }

        override fun onCustomCommand(
            session: MediaSession,
            controller: MediaSession.ControllerInfo,
            customCommand: SessionCommand,
            args: android.os.Bundle,
        ): ListenableFuture<SessionResult> {
            when (customCommand.customAction) {
                CMD_SHUFFLE -> {
                    shuffle()
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
                CMD_TOGGLE -> {
                    val player = exo
                    when {
                        player == null -> shuffle()
                        player.mediaItemCount == 0 -> shuffle()
                        player.isPlaying -> player.pause()
                        else -> player.play()
                    }
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
                CMD_STOP -> {
                    exo?.stop()
                    pauseAllPlayersAndStopSelf()
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
                CMD_STAR -> {
                    val st = current
                    when (st) {
                        null -> {}
                        else -> scope.launch {
                            favStore.toggleFav(st)
                            refreshCustomLayout()
                            refreshWidget(current, exo?.isPlaying == true, favStore.currentFavUuids())
                            syncNow()
                        }
                    }
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
                CMD_SCOPE -> {
                    val target = args.getString("scope")?.let { runCatching { Scope.valueOf(it) }.getOrNull() }
                    scope.launch {
                        val next = target ?: when (favStore.currentScope()) {
                            Scope.ALL -> Scope.FAVS
                            Scope.FAVS -> Scope.ALL
                        }
                        favStore.setScope(next)
                        refreshCustomLayout()
                        refreshWidget(current, exo?.isPlaying == true, favStore.currentFavUuids())
                        // the scope is carried by the account: setScope stamps it, and
                        // only this call takes it to the other devices.
                        syncNow()
                        shuffle()
                    }
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
                CMD_CLEAR_FILTER -> {
                    scope.launch {
                        favStore.setFilter(emptySet())
                        // the countries pulled for the old filter stay in the cache —
                        // they cost nothing and re-selecting that country is instant.
                        refreshCustomLayout()
                        // same as the scope: only this call takes the change to the
                        // other devices, and the filter is shared across all of them.
                        syncNow()
                        shuffle()
                    }
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
                CMD_SYNC_UI -> {
                    launchSyncActivity()
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
                CMD_PLAY_UUID -> {
                    val uuid = args.getString(ARG_UUID).orEmpty()
                    scope.launch {
                        // a linear scan over the whole catalogue must not run on the
                        // ui thread — withReadyCatalog() returns synchronously with
                        // no dispatcher change once the list is warm, which is the
                        // common case by the time a tap can happen.
                        val station = withContext(Dispatchers.Default) {
                            withReadyCatalog().firstOrNull { it.uuid == uuid }
                        }
                        when (station) {
                            // the catalogue the screen listed and the one the
                            // service holds can differ after a refresh; a tap on
                            // a station that is gone must do nothing, not crash.
                            null -> Log.w("r4dio", "play requested for unknown station $uuid")
                            else -> main.post { playPick(station) }
                        }
                    }
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
            }
            return Futures.immediateFuture(SessionResult(SessionResult.RESULT_ERROR_NOT_SUPPORTED))
        }
    }
}
