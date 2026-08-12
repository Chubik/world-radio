package net.vchub.r4dio

import android.Manifest
import android.app.AlertDialog
import android.content.ComponentName
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.net.Uri
import android.os.Bundle
import android.provider.Settings
import android.widget.Toast
import android.view.View
import android.view.WindowManager
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import com.google.common.util.concurrent.MoreExecutors
import kotlinx.coroutines.launch

/** Splits the packed artist string ("SA · MP3 · 128k") into country and codec. */
fun parseArtist(artist: String?): Pair<String?, String?> {
    val parts = artist?.split("·")?.map { it.trim() }?.filter { it.isNotEmpty() } ?: emptyList()
    val country = parts.getOrNull(0)
    val codec = parts.getOrNull(1)
    return country to codec
}

class MainActivity : ComponentActivity() {
    private var controllerFuture: ListenableFuture<MediaController>? = null
    private var controller: MediaController? = null
    private var playerListener: Player.Listener? = null
    private var fav = false
    private var scope = "all"
    private var favCount = 0
    private var hiddenCount = 0
    private var playableCount = 0
    private var catalogueSize = 0
    private var catalogueGrowing = false
    private var catalogLoaded = false
    private var filterCountries: List<String> = emptyList()
    private var released = false
    private val favStore by lazy { FavStore(this) }

    private val requestPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) {
            connect()
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        wireControls()
        when (needsNotificationPermission()) {
            true -> requestPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
            false -> connect()
        }
    }

    private fun wireControls() {
        findViewById<View>(R.id.stage).setOnClickListener { send(CMD_SHUFFLE) }
        findViewById<View>(R.id.btn_play).setOnClickListener { send(CMD_TOGGLE) }
        findViewById<View>(R.id.btn_star).setOnClickListener { send(CMD_STAR) }
        findViewById<View>(R.id.btn_scope).setOnClickListener { send(CMD_SCOPE) }
        findViewById<View>(R.id.btn_stop).setOnClickListener { send(CMD_STOP) }
        findViewById<View>(R.id.btn_sync).setOnClickListener {
            startActivity(Intent(this, SyncActivity::class.java))
        }
        // the filter is shared across every device on the account, so clearing it
        // is not a local view toggle — it is confirmed rather than done on a tap
        // that is easy to make by accident reaching for the screen in a car.
        findViewById<View?>(R.id.filter_pill)?.setOnClickListener {
            if (filterCountries.isEmpty()) {
                return@setOnClickListener
            }
            val codes = filterCountries.joinToString("·")
            AlertDialog.Builder(this, R.style.R4dioDialog)
                .setTitle(R.string.filter_clear_title)
                .setMessage(getString(R.string.filter_clear_body, codes))
                .setPositiveButton(R.string.filter_clear_yes) { _, _ -> send(CMD_CLEAR_FILTER) }
                .setNegativeButton(getString(R.string.filter_clear_no, codes), null)
                .show()
        }
        findViewById<View>(R.id.awake_pill).setOnClickListener {
            lifecycleScope.launch {
                val next = nextKeepAwake(favStore.currentKeepAwake())
                favStore.setKeepAwake(next)
                applyKeepAwake(next)
            }
        }
        // android grants this one only from its own settings screen, so the pill
        // opens that rather than pretending it can ask here.
        // null in landscape, which carries a shorter pill row: every pill lookup
        // here is optional for that reason, not because the id might be missing.
        findViewById<View?>(R.id.overlay_pill)?.setOnClickListener {
            if (canDrawOverlay(this)) {
                Toast.makeText(this, R.string.home_overlay_desc, Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            Toast.makeText(this, R.string.overlay_ask, Toast.LENGTH_LONG).show()
            runCatching {
                startActivity(
                    Intent(
                        Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                        Uri.parse("package:$packageName"),
                    )
                )
            }
        }
    }

    /**
     * FLAG_KEEP_SCREEN_ON rather than a wake lock: the system drops it for us the
     * moment this window stops being visible, so a forgotten toggle cannot hold the
     * screen on behind another app.
     */
    private fun applyKeepAwake(on: Boolean) {
        when (on) {
            true -> window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            false -> window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        }
        findViewById<TextView>(R.id.awake_pill).apply {
            text = keepAwakeLabel(on)
            setTextColor(getColor(if (on) R.color.amber_hi else R.color.dim))
            setBackgroundResource(if (on) R.drawable.bg_pill_on else R.drawable.bg_pill)
        }
    }

    // the flag lives on the window, and a window rebuilt after a rotation or a trip
    // through another app comes back without it — so it is reapplied on every resume,
    // not just once at creation.
    override fun onResume() {
        super.onResume()
        // the overlay exists for playback the user cannot see, so the service
        // has to know whether this screen is the thing in front.
        StationToast.appIsInForeground = true
        // read back from the system rather than remembered: the user may have
        // granted or revoked it in settings while we were away, and a pill that
        // disagrees with the permission is worse than no pill.
        applyOverlayPill(canDrawOverlay(this))
        lifecycleScope.launch { applyKeepAwake(favStore.currentKeepAwake()) }
    }

    private fun applyOverlayPill(on: Boolean) {
        findViewById<TextView?>(R.id.overlay_pill)?.apply {
            setText(if (on) R.string.home_overlay_on else R.string.home_overlay_off)
            setTextColor(getColor(if (on) R.color.amber_hi else R.color.dim))
            setBackgroundResource(if (on) R.drawable.bg_pill_on else R.drawable.bg_pill)
        }
    }

    override fun onPause() {
        super.onPause()
        StationToast.appIsInForeground = false
    }

    private fun send(action: String) {
        controller?.sendCustomCommand(SessionCommand(action, Bundle.EMPTY), Bundle.EMPTY)
    }

    private fun connect() {
        val token = SessionToken(this, ComponentName(this, PlaybackService::class.java))
        val sessionListener = object : MediaController.Listener {
            override fun onExtrasChanged(session: MediaController, extras: Bundle) {
                if (released) {
                    return
                }
                readExtras(extras)
                render()
            }
        }
        val future = MediaController.Builder(this, token).setListener(sessionListener).buildAsync()
        controllerFuture = future
        // the future can resolve after onDestroy (rotation right after launch): without the
        // guard we would attach a player listener to a controller this activity no longer owns.
        future.addListener({
            val c = runCatching { future.get() }.getOrNull()
            when {
                released -> {}
                c == null -> render()
                else -> onConnected(c)
            }
        }, MoreExecutors.directExecutor())
    }

    private fun onConnected(c: MediaController) {
        controller = c
        readExtras(c.sessionExtras)
        val l = object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) = render()
            override fun onMediaMetadataChanged(mediaMetadata: MediaMetadata) = render()
        }
        playerListener = l
        c.addListener(l)
        when (c.mediaItemCount == 0) {
            true -> send(CMD_SHUFFLE)
            false -> {}
        }
        render()
    }

    private fun readExtras(extras: Bundle) {
        fav = extras.getBoolean(EXTRA_FAV, false)
        scope = extras.getString(EXTRA_SCOPE, "all") ?: "all"
        favCount = extras.getInt(EXTRA_FAV_COUNT, 0)
        hiddenCount = extras.getInt(EXTRA_HIDDEN_COUNT, 0)
        playableCount = extras.getInt(EXTRA_PLAYABLE_COUNT, 0)
        catalogLoaded = extras.getBoolean(EXTRA_CATALOG_LOADED, false)
        catalogueSize = extras.getInt(EXTRA_CATALOG_SIZE, 0)
        catalogueGrowing = extras.getBoolean(EXTRA_CATALOG_GROWING, false)
        filterCountries = extras.getStringArray(EXTRA_FILTER_COUNTRIES)?.toList() ?: emptyList()
    }

    /** the two reasons shuffle has nothing to pick, each with its own message. */
    private fun warnMessage(): Int? = when {
        scope == "favs" && favCount == 0 -> R.string.home_warn_no_favs
        isAllHiddenWarn(playableCount, hiddenCount, scope, catalogLoaded) -> R.string.home_warn_all_hidden
        else -> null
    }

    private fun render() {
        val c = controller
        renderPlayback(c?.isPlaying ?: false)
        renderStation(c)
        renderScope()
        renderHidden()
        renderFilter()
        renderCatalogue()
        renderFav()
        renderHero()
        renderPlayButton(c?.isPlaying ?: false)
        renderStarButton()
        renderScopeButton()
    }

    private fun renderPlayback(isPlaying: Boolean) {
        findViewById<TextView>(R.id.kicker_label).text =
            getString(if (isPlaying) R.string.home_now_playing else R.string.home_paused)
        findViewById<View>(R.id.eq).visibility = if (isPlaying) View.VISIBLE else View.GONE
        val live = findViewById<TextView>(R.id.kicker_live)
        when (isPlaying) {
            true -> {
                live.text = getString(R.string.home_live)
                live.setTextColor(getColor(R.color.olive))
            }
            false -> {
                live.text = getString(R.string.home_off_air)
                live.setTextColor(getColor(R.color.mute))
            }
        }
    }

    private fun renderStation(c: MediaController?) {
        val metadata = c?.mediaMetadata
        val name = metadata?.station?.toString()?.ifBlank { null }
            ?: metadata?.title?.toString()?.ifBlank { null }
            ?: getString(R.string.home_idle)
        findViewById<TextView>(R.id.station_name).text = name

        val (country, codec) = parseArtist(metadata?.artist?.toString())
        findViewById<TextView>(R.id.ctx_country).text = country.orEmpty()
        // the separator lives on the codec so an absent country never leaves a dangling dot
        val codecText = when {
            codec.isNullOrBlank() -> ""
            country.isNullOrBlank() -> codec
            else -> "· $codec"
        }
        findViewById<TextView>(R.id.ctx_codec).text = codecText
    }

    private fun renderScope() {
        val pill = findViewById<TextView>(R.id.scope_pill)
        when (scope) {
            "favs" -> {
                pill.text = when (favCount) {
                    0 -> getString(R.string.home_scope_favs)
                    else -> getString(R.string.home_scope_favs_n, favCount)
                }
                pill.setBackgroundResource(R.drawable.bg_pill_on)
                pill.setTextColor(getColor(R.color.amber_hi))
            }
            else -> {
                pill.text = getString(R.string.home_scope_all)
                pill.setBackgroundResource(R.drawable.bg_pill)
                pill.setTextColor(getColor(R.color.dim))
            }
        }
    }

    private fun renderHidden() {
        val pill = findViewById<TextView>(R.id.hidden_pill)
        when (showsHiddenPill(hiddenCount, scope)) {
            true -> {
                pill.text = getString(R.string.home_countries_n, hiddenCount)
                pill.visibility = View.VISIBLE
            }
            false -> pill.visibility = View.GONE
        }
    }

    private fun renderCatalogue() {
        val pill = findViewById<TextView?>(R.id.catalogue_pill) ?: return
        when (val label = catalogueLabel(catalogueSize, catalogueGrowing)) {
            "" -> pill.visibility = View.GONE
            else -> {
                pill.text = label
                pill.visibility = View.VISIBLE
            }
        }
    }

    private fun renderFilter() {
        val pill = findViewById<TextView?>(R.id.filter_pill) ?: return
        when (val label = filterPillLabel(filterCountries, scope)) {
            null -> pill.visibility = View.GONE
            else -> {
                val inForce = filterIsInForce(filterCountries, scope)
                pill.text = label
                pill.visibility = View.VISIBLE
                // dimmed rather than hidden while favourites bypass it: the filter
                // is still set, and a pill that vanished read as "it is gone".
                pill.setBackgroundResource(if (inForce) R.drawable.bg_pill_on else R.drawable.bg_pill)
                pill.setTextColor(getColor(if (inForce) R.color.amber_hi else R.color.dim))
            }
        }
    }

    private fun renderFav() {
        val ctxFav = findViewById<TextView>(R.id.ctx_fav)
        val hasContext = findViewById<TextView>(R.id.ctx_country).text.isNotBlank() ||
            findViewById<TextView>(R.id.ctx_codec).text.isNotBlank()
        val prefix = if (hasContext) "· " else ""
        when (fav) {
            true -> {
                ctxFav.text = prefix + getString(R.string.home_fav_yes)
                ctxFav.setTextColor(getColor(R.color.amber))
            }
            false -> {
                ctxFav.text = prefix + getString(R.string.home_fav_no)
                ctxFav.setTextColor(getColor(R.color.mute))
            }
        }
    }

    private fun renderHero() {
        val ring = findViewById<View>(R.id.hero_ring)
        val sub = findViewById<TextView>(R.id.hero_sub)
        val glyph = findViewById<ImageView>(R.id.hero_glyph)
        val label = findViewById<TextView>(R.id.hero_label)
        val warn = warnMessage()
        val tone = if (warn != null) R.color.danger else R.color.amber_hi
        glyph.setColorFilter(getColor(tone))
        label.setTextColor(getColor(tone))
        when (warn) {
            null -> {
                ring.setBackgroundResource(R.drawable.bg_hero_ring)
                val sc = if (scope == "favs") R.string.home_shuffle_sub_favs else R.string.home_shuffle_sub_all
                sub.text = getString(sc)
                sub.setTextColor(getColor(R.color.dim))
            }
            else -> {
                ring.setBackgroundResource(R.drawable.bg_hero_ring_warn)
                sub.text = getString(warn)
                sub.setTextColor(getColor(R.color.danger))
            }
        }
    }

    private fun renderPlayButton(isPlaying: Boolean) {
        val ico = findViewById<ImageView>(R.id.btn_play_ico)
        val label = findViewById<TextView>(R.id.btn_play_label)
        when (isPlaying) {
            true -> {
                ico.setImageResource(R.drawable.ic_pause)
                label.text = getString(R.string.home_pause)
            }
            false -> {
                ico.setImageResource(R.drawable.ic_play)
                label.text = getString(R.string.home_play)
            }
        }
    }

    private fun renderStarButton() {
        val btn = findViewById<LinearLayout>(R.id.btn_star)
        val ico = findViewById<ImageView>(R.id.btn_star_ico)
        val label = findViewById<TextView>(R.id.btn_star_label)
        when (fav) {
            true -> {
                ico.setImageResource(R.drawable.ic_star)
                label.text = getString(R.string.home_starred)
                btn.setBackgroundResource(R.drawable.bg_sec_btn_on)
            }
            false -> {
                ico.setImageResource(R.drawable.ic_star_outline)
                label.text = getString(R.string.home_star)
                btn.setBackgroundResource(R.drawable.bg_sec_btn)
            }
        }
    }

    private fun renderScopeButton() {
        val ico = findViewById<ImageView>(R.id.btn_scope_ico)
        when (scope) {
            "favs" -> ico.setImageResource(R.drawable.ic_scope_favs)
            else -> ico.setImageResource(R.drawable.ic_scope_all)
        }
    }

    override fun onDestroy() {
        released = true
        playerListener?.let { controller?.removeListener(it) }
        playerListener = null
        controller = null
        controllerFuture?.let { MediaController.releaseFuture(it) }
        controllerFuture = null
        super.onDestroy()
    }

    private fun needsNotificationPermission(): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            return false
        }
        return ContextCompat.checkSelfPermission(
            this,
            Manifest.permission.POST_NOTIFICATIONS,
        ) != PackageManager.PERMISSION_GRANTED
    }
}
