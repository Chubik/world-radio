package net.vchub.r4dio.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.vchub.r4dio.CMD_CLEAR_FILTER
import net.vchub.r4dio.CMD_SCOPE
import androidx.annotation.StringRes
import androidx.compose.ui.res.stringResource
import net.vchub.r4dio.R
import net.vchub.r4dio.CMD_SHUFFLE
import net.vchub.r4dio.CMD_STAR
import net.vchub.r4dio.CMD_STOP
import net.vchub.r4dio.CMD_TOGGLE
import net.vchub.r4dio.Station

enum class Tab(val icon: String, @StringRes val label: Int) {
    HOME("⇄", R.string.tab_home),
    CATALOG("⌕", R.string.tab_catalog),
    LIBRARY("★", R.string.tab_library),
    SETTINGS("⚙", R.string.tab_settings),
}

/**
 * home already shows the station at full size, so the strip would be a second
 * copy of the same fact. everywhere else it is the only way back to it.
 */
fun showsMiniPlayer(tab: Tab, stationName: String): Boolean =
    tab != Tab.HOME && stationName.isNotBlank()

/** the catalogue as the shell sees it: an unread cache and an empty one are not
 *  the same thing, and the screen must be able to tell them apart. */
data class CatalogState(
    val stations: List<Station> = emptyList(),
    val loading: Boolean = true,
)

/**
 * the three library lists, resolved by the host: favourites come from the fav
 * cache whole, blocked are uuids named from the catalogue, and history is the
 * local play list.
 */
data class LibraryState(
    val favourites: List<Station> = emptyList(),
    val blocked: List<Station> = emptyList(),
    val history: List<Station> = emptyList(),
)

/**
 * the four-tab shell every screen lives in. tab choice is rememberSaveable so
 * a rotation does not throw the user back to home, unlike plain remember.
 */
@Composable
fun R4dioApp(
    state: UiState,
    send: (String) -> Unit,
    onOpenSync: () -> Unit,
    keepAwake: Boolean = false,
    overlayOn: Boolean = false,
    onKeepAwake: (() -> Unit)? = null,
    onOverlay: (() -> Unit)? = null,
    fillOnMobile: Boolean = true,
    onFillOnMobile: (() -> Unit)? = null,
    theme: String = "",
    hiddenCountries: Set<String> = emptySet(),
    onTheme: (String) -> Unit = {},
    onShowCountry: (String) -> Unit = {},
    // clearing the filter changes every device on the account, so the host can
    // put a confirmation in front of it instead of sending the command straight.
    onClearFilter: () -> Unit = { send(CMD_CLEAR_FILTER) },
    catalog: CatalogState = CatalogState(loading = false),
    library: LibraryState = LibraryState(),
    onClearHistory: () -> Unit = {},
    /** blocks whatever is playing; the screen holds its uuid, the host the store. */
    onBlockPlaying: () -> Unit = {},
    favourites: Set<String> = emptySet(),
    blocked: Set<String> = emptySet(),
    onPlay: (Station) -> Unit = {},
    onStar: (Station) -> Unit = {},
    onBlock: (Station) -> Unit = {},
    // the host re-reads the 10mb cache when the catalogue tab comes forward,
    // so a background top-up shows up without a restart.
    onCatalogShown: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    val c = R4dioTokens.colors
    var tab by rememberSaveable { mutableStateOf(Tab.HOME) }
    var nowPlaying by rememberSaveable { mutableStateOf(false) }
    // library needs it too, to put names on blocked uuids — without this a user
    // who opens library first sees bare ids, which is exactly when the name
    // matters most.
    LaunchedEffect(tab, state.catalogueSize) {
        if (tab == Tab.CATALOG || tab == Tab.LIBRARY) onCatalogShown()
    }
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Color(c.bg)),
    ) {
        Box(modifier = Modifier.weight(1f)) {
            when (tab) {
                Tab.HOME -> HomeScreen(
                    state = state,
                    onShuffle = { send(CMD_SHUFFLE) },
                    onToggle = { send(CMD_TOGGLE) },
                    onStar = { send(CMD_STAR) },
                    onScope = { send(CMD_SCOPE) },
                    onStop = { send(CMD_STOP) },
                    onSync = onOpenSync,
                    onClearFilter = onClearFilter,
                    keepAwake = keepAwake,
                    overlayOn = overlayOn,
                    onKeepAwake = onKeepAwake,
                    onOverlay = onOverlay,
                )
                Tab.CATALOG -> CatalogScreen(
                    stations = catalog.stations,
                    favourites = favourites,
                    blocked = blocked,
                    onPlay = onPlay,
                    onStar = onStar,
                    onBlock = onBlock,
                    loading = catalog.loading,
                )
                Tab.LIBRARY -> LibraryScreen(
                    favourites = library.favourites,
                    blocked = library.blocked,
                    history = library.history,
                    favouriteUuids = favourites,
                    blockedUuids = blocked,
                    onPlay = onPlay,
                    onStar = onStar,
                    onBlock = onBlock,
                    onClearHistory = onClearHistory,
                )
                Tab.SETTINGS -> SettingsScreen(
                    theme = theme,
                    hiddenCountries = hiddenCountries,
                    fillOnMobile = fillOnMobile,
                    onTheme = onTheme,
                    onShowCountry = onShowCountry,
                    onFillOnMobile = onFillOnMobile,
                    onOpenSync = onOpenSync,
                )
            }
        }
        if (showsMiniPlayer(tab, state.stationName)) {
            MiniPlayer(state) { nowPlaying = true }
        }
        TabBar(tab) { tab = it }
    }

    // over the whole shell rather than inside the tab body: now playing is not a
    // tab, and covering the tab bar is what makes it read as a full screen.
    if (nowPlaying) {
        // a station that stops while the screen is open leaves nothing to show.
        BackHandler { nowPlaying = false }
        NowPlayingScreen(
            state = state,
            onToggle = { send(CMD_TOGGLE) },
            onShuffle = { send(CMD_SHUFFLE) },
            onStar = { send(CMD_STAR) },
            onBlock = onBlockPlaying,
            onStop = {
                send(CMD_STOP)
                nowPlaying = false
            },
            onClose = { nowPlaying = false },
        )
    }
}

/** now-playing, reachable from every tab but home. tapping it opens the full screen. */
@Composable
private fun MiniPlayer(state: UiState, onOpen: () -> Unit) {
    val c = R4dioTokens.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(48.dp)
            .background(Color(c.panel()))
            .border(1.dp, Color(c.rule()))
            .clickable(onClick = onOpen)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = if (state.isPlaying) "▶" else "⏸",
            color = Color(c.accent),
            fontSize = 13.sp,
            fontFamily = MonoFamily,
        )
        Text(
            text = state.stationName,
            color = Color(c.fg),
            fontSize = 13.sp,
            fontFamily = MonoFamily,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f).padding(start = 10.dp),
        )
    }
}

@Composable
private fun TabBar(current: Tab, onSelect: (Tab) -> Unit) {
    val c = R4dioTokens.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(56.dp)
            .background(Color(c.panel()))
            .border(1.dp, Color(c.rule())),
    ) {
        Tab.entries.forEach { tab ->
            val on = tab == current
            Column(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxSize()
                    .clickable { onSelect(tab) }
                    .padding(vertical = 6.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                Text(
                    text = tab.icon,
                    color = Color(if (on) c.accent else c.dim),
                    fontSize = 16.sp,
                    fontFamily = MonoFamily,
                )
                Text(
                    text = stringResource(tab.label),
                    color = Color(if (on) c.accent else c.dim),
                    fontSize = 9.sp,
                    fontWeight = FontWeight.SemiBold,
                    fontFamily = MonoFamily,
                    letterSpacing = 0.1.em,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
        }
    }
}

