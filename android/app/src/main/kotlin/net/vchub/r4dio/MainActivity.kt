package net.vchub.r4dio

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.view.WindowManager
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.vchub.r4dio.ui.DEFAULT_THEME
import net.vchub.r4dio.ui.MonoFamily
import net.vchub.r4dio.ui.PlayerConnection
import net.vchub.r4dio.ui.R4dioApp
import net.vchub.r4dio.ui.R4dioTheme
import net.vchub.r4dio.ui.R4dioTokens
import net.vchub.r4dio.ui.mediaControllerConnector
import net.vchub.r4dio.ui.resolveTheme

class MainActivity : ComponentActivity() {
    // the connector needs the connection to fold callbacks into, and the
    // connection needs the connector — hence the lambda indirection. safe
    // because the connector never calls conn() synchronously.
    private val connection: PlayerConnection by lazy {
        PlayerConnection(mediaControllerConnector(this) { connection })
    }
    private val favStore by lazy { FavStore(this) }

    // READ ONLY. write/merge from here would race the service's read-modify-write
    // and lose whatever a background top-up had just merged. the service owns
    // every write; this side only ever calls read().
    private val catalogCache by lazy { CatalogCache(filesDir) }
    private var catalog by mutableStateOf(net.vchub.r4dio.ui.CatalogState())

    // read back from the system on every resume rather than remembered: the user
    // may have granted or revoked it in settings while we were away, and a pill
    // that disagrees with the permission is worse than no pill.
    private var overlayOn by mutableStateOf(false)
    private var keepAwake by mutableStateOf(false)

    private val requestPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) {
            connection.connect()
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            val state by connection.state.collectAsStateWithLifecycle()
            val synced by favStore.theme.collectAsStateWithLifecycle(initialValue = "")
            val favourites by favStore.favUuids.collectAsStateWithLifecycle(initialValue = emptySet())
            val blocked by favStore.blockedUuids.collectAsStateWithLifecycle(initialValue = emptySet())
            // the initial value matches the store's own default, so the pill
            // never shows "off" for an instant before the real value lands.
            val fillOnMobile by favStore.fillOnMobile.collectAsStateWithLifecycle(initialValue = true)
            val slug = resolveTheme(synced, DEFAULT_THEME)
            var clearing by remember { mutableStateOf(false) }
            R4dioTheme(slug) {
                R4dioApp(
                    state = state,
                    send = { connection.send(it) },
                    onOpenSync = { startActivity(Intent(this, SyncActivity::class.java)) },
                    keepAwake = keepAwake,
                    overlayOn = overlayOn,
                    onKeepAwake = ::toggleKeepAwake,
                    onOverlay = ::askOverlay,
                    fillOnMobile = fillOnMobile,
                    onFillOnMobile = {
                        lifecycleScope.launch { favStore.setFillOnMobile(!fillOnMobile) }
                    },
                    onClearFilter = { clearing = state.filterCountries.isNotEmpty() },
                    catalog = catalog,
                    favourites = favourites,
                    blocked = blocked,
                    onPlay = ::playStation,
                    onStar = { station -> lifecycleScope.launch { favStore.toggleFav(station) } },
                    onBlock = { station -> lifecycleScope.launch { favStore.toggleBlocked(station.uuid) } },
                    onCatalogShown = ::loadCatalog,
                    // the compose tree owns the whole window, so the inset the xml
                    // root used to take with fitsSystemWindows is applied here.
                    modifier = Modifier.windowInsetsPadding(WindowInsets.systemBars),
                )
                if (clearing) {
                    ClearFilterDialog(
                        codes = state.filterCountries.joinToString("·"),
                        onConfirm = {
                            clearing = false
                            connection.send(CMD_CLEAR_FILTER)
                        },
                        onDismiss = { clearing = false },
                    )
                }
            }
        }
        when (needsNotificationPermission()) {
            true -> requestPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
            false -> connection.connect()
        }
    }

    /**
     * the cache is 10mb and holds ~59k stations, so the read is never instant —
     * it goes to IO and the screen shows its loading state until it lands. read
     * only: see [catalogCache].
     */
    private fun loadCatalog() {
        lifecycleScope.launch {
            val stations = withContext(Dispatchers.IO) { catalogCache.read() }
            catalog = net.vchub.r4dio.ui.CatalogState(stations = stations, loading = false)
        }
    }

    /** the service owns the catalogue it plays from, so the tap sends a uuid
     *  rather than a url — the station the screen listed may have been replaced
     *  by a refresh, and the service resolves that. */
    private fun playStation(station: Station) {
        connection.send(CMD_PLAY_UUID, Bundle().apply { putString(ARG_UUID, station.uuid) })
    }

    private fun toggleKeepAwake() {
        lifecycleScope.launch {
            val next = nextKeepAwake(favStore.currentKeepAwake())
            favStore.setKeepAwake(next)
            applyKeepAwake(next)
        }
    }

    // android grants this one only from its own settings screen, so the pill
    // opens that rather than pretending it can ask here.
    private fun askOverlay() {
        if (canDrawOverlay(this)) {
            Toast.makeText(this, R.string.home_overlay_desc, Toast.LENGTH_SHORT).show()
            return
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

    /**
     * FLAG_KEEP_SCREEN_ON rather than a wake lock: the system drops it for us the
     * moment this window stops being visible, so a forgotten toggle cannot hold the
     * screen on behind another app.
     */
    private fun applyKeepAwake(on: Boolean) {
        keepAwake = on
        when (on) {
            true -> window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            false -> window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
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
        overlayOn = canDrawOverlay(this)
        lifecycleScope.launch { applyKeepAwake(favStore.currentKeepAwake()) }
    }

    override fun onPause() {
        super.onPause()
        StationToast.appIsInForeground = false
    }

    override fun onDestroy() {
        connection.release()
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

/**
 * the filter is shared across every device on the account, so clearing it is not
 * a local view toggle — it is confirmed rather than done on a tap that is easy to
 * make by accident reaching for the screen in a car.
 */
@Composable
private fun ClearFilterDialog(codes: String, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    val c = R4dioTokens.colors
    val context = LocalContext.current
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = Color(c.bg),
        titleContentColor = Color(c.accent),
        textContentColor = Color(c.fg),
        title = {
            Text(
                text = stringResource(R.string.filter_clear_title),
                color = Color(c.accent),
                fontSize = 16.sp,
                fontWeight = FontWeight.Bold,
                fontFamily = MonoFamily,
            )
        },
        text = {
            Text(
                text = context.getString(R.string.filter_clear_body, codes),
                color = Color(c.fg),
                fontSize = 13.sp,
                fontFamily = MonoFamily,
            )
        },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(
                    text = stringResource(R.string.filter_clear_yes),
                    color = Color(c.accent),
                    fontSize = 13.sp,
                    fontFamily = MonoFamily,
                )
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(
                    text = context.getString(R.string.filter_clear_no, codes),
                    color = Color(c.dim),
                    fontSize = 13.sp,
                    fontFamily = MonoFamily,
                )
            }
        },
    )
}
