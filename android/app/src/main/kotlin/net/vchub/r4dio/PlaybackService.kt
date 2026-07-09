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

const val CMD_SHUFFLE = "net.vchub.r4dio.SHUFFLE"
const val CMD_STAR = "net.vchub.r4dio.STAR"
const val CMD_SCOPE = "net.vchub.r4dio.SCOPE"
const val CMD_STOP = "net.vchub.r4dio.STOP"
const val CMD_SYNC_UI = "net.vchub.r4dio.SYNC_UI"

class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null
    private var exo: ExoPlayer? = null
    private val catalog = Catalog()
    @Volatile private var stations: List<Station> = emptyList()
    @Volatile private var current: Station? = null
    @Volatile private var mirrorSeq: Long = 0
    @Volatile private var applyingMirror: Boolean = false
    private val artwork: ByteArray by lazy { crtArtworkPng() }
    private var mirrorJob: Job? = null
    private val main = Handler(Looper.getMainLooper())
    private val favStore by lazy { FavStore(this) }
    private val syncClient = SyncClient()
    private val mirrorClient = MirrorClient()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    private val shuffleCommand = SessionCommand(CMD_SHUFFLE, android.os.Bundle.EMPTY)
    private val starCommand = SessionCommand(CMD_STAR, android.os.Bundle.EMPTY)
    private val scopeCommand = SessionCommand(CMD_SCOPE, android.os.Bundle.EMPTY)
    private val stopCommand = SessionCommand(CMD_STOP, android.os.Bundle.EMPTY)
    private val syncUiCommand = SessionCommand(CMD_SYNC_UI, android.os.Bundle.EMPTY)

    private val shuffleButton = CommandButton.Builder(CommandButton.ICON_SHUFFLE_ON)
        .setDisplayName("shuffle")
        .setCustomIconResId(R.drawable.ic_shuffle)
        .setSessionCommand(shuffleCommand)
        .build()

    private val stopButton = CommandButton.Builder(CommandButton.ICON_STOP)
        .setDisplayName("stop")
        .setSessionCommand(stopCommand)
        .build()

    private val syncButton = CommandButton.Builder(CommandButton.ICON_UNDEFINED)
        .setDisplayName("sync")
        .setCustomIconResId(R.drawable.ic_sync)
        .setSessionCommand(syncUiCommand)
        .build()

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
        session?.setCustomLayout(listOf(shuffleButton, starButton(isFav), syncButton, stopButton))
    }

    override fun onCreate() {
        super.onCreate()
        val player = ExoPlayer.Builder(this).build()
        player.addListener(object : androidx.media3.common.Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                current?.let { RadioWidgetProvider.refresh(this@PlaybackService, it.name, isPlaying) }
            }

            override fun onPlayerError(error: androidx.media3.common.PlaybackException) {
                Log.w("r4dio", "playback error: ${error.errorCodeName}, skipping station")
                shuffle()
            }
        })
        exo = player
        session = MediaSession.Builder(this, player)
            .setCallback(Callback())
            .build()
        val provider = androidx.media3.session.DefaultMediaNotificationProvider.Builder(this).build()
        provider.setSmallIcon(R.drawable.ic_stat_r4dio)
        setMediaNotificationProvider(provider)
        loadStations()
        syncNow()
        startMirrorListener()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_WIDGET_SHUFFLE -> shuffle()
            ACTION_WIDGET_TOGGLE -> exo?.let { if (it.isPlaying) it.pause() else it.play() }
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
            val fetched = catalog.fetchStations()
            stations = fetched
            Log.i("r4dio", "loaded ${fetched.size} stations")
            val pick = pickRandom(fetched) ?: return@thread
            main.post { playPick(pick) }
        }
    }

    private fun launchSyncActivity() {
        val intent = android.content.Intent(this, SyncActivity::class.java)
            .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
        val pending = android.app.PendingIntent.getActivity(
            this,
            0,
            intent,
            android.app.PendingIntent.FLAG_IMMUTABLE or android.app.PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val options = when (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            true -> android.app.ActivityOptions.makeBasic()
                .setPendingIntentBackgroundActivityStartMode(
                    android.app.ActivityOptions.MODE_BACKGROUND_ACTIVITY_START_ALLOWED,
                )
                .toBundle()
            false -> null
        }
        runCatching { pending.send(this, 0, null, null, null, null, options) }
    }

    private fun syncNow() {
        scope.launch {
            val key = favStore.syncKey() ?: return@launch
            val local = SyncData(
                favs = favStore.currentFavUuids().toList(),
                blocked = favStore.currentBlocked().toList(),
            )
            val merged = withContext(Dispatchers.IO) { syncClient.push(key, local) } ?: return@launch
            favStore.applyMerged(merged.favs.toSet(), merged.blocked.toSet())
            refreshCustomLayout()
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
                                true -> scope.launch { onMirrorEvent(evt, myId) }
                            }
                        }
                        delay(3_000)
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
        if (isExcluded(station)) {
            return
        }
        when (exo?.isPlaying) {
            true -> {
                applyingMirror = true
                playPick(station)
                applyingMirror = false
            }
            else -> {
                current = station
                RadioWidgetProvider.refresh(this, evt.name, false)
            }
        }
    }

    private fun shuffle() {
        scope.launch {
            val sc = favStore.currentScope()
            val favs = favStore.currentCachedFavs()
            val cat = withReadyCatalog()
            val pick = pickForScope(sc, cat, favs)
            when (pick) {
                null -> Log.i("r4dio", "shuffle: nothing to play for scope $sc")
                else -> playPick(pick)
            }
        }
    }

    private suspend fun withReadyCatalog(): List<Station> {
        val cur = stations
        if (cur.isNotEmpty()) return cur
        val fetched = withContext(Dispatchers.IO) {
            catalog.fetchStations()
        }
        stations = fetched
        Log.i("r4dio", "fetched ${fetched.size} stations for shuffle")
        return fetched
    }

    private fun playPick(pick: Station) {
        val player = exo ?: return
        current = pick
        RadioWidgetProvider.refresh(this, pick.name, true)
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
                scope.launch { refreshCustomLayout() }
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
                    .add(starCommand)
                    .add(scopeCommand)
                    .add(stopCommand)
                    .add(syncUiCommand)
                    .build()
            val playerCommands =
                MediaSession.ConnectionResult.DEFAULT_PLAYER_COMMANDS.buildUpon()
                    .remove(androidx.media3.common.Player.COMMAND_SEEK_TO_NEXT)
                    .remove(androidx.media3.common.Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM)
                    .remove(androidx.media3.common.Player.COMMAND_SEEK_TO_PREVIOUS)
                    .remove(androidx.media3.common.Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM)
                    .build()
            return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
                .setAvailableSessionCommands(sessionCommands)
                .setAvailablePlayerCommands(playerCommands)
                .setCustomLayout(listOf(shuffleButton, starButton(false), syncButton, stopButton))
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
                        shuffle()
                    }
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
                CMD_SYNC_UI -> {
                    launchSyncActivity()
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
            }
            return Futures.immediateFuture(SessionResult(SessionResult.RESULT_ERROR_NOT_SUPPORTED))
        }
    }
}
