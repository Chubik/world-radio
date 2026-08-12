package net.vchub.r4dio.ui

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
import net.vchub.r4dio.CMD_SHUFFLE
import net.vchub.r4dio.CMD_STAR
import net.vchub.r4dio.CMD_STOP
import net.vchub.r4dio.CMD_TOGGLE

enum class Tab(val icon: String, val label: String) {
    HOME("⇄", "HOME"),
    CATALOG("⌕", "CATALOG"),
    LIBRARY("★", "LIBRARY"),
    SETTINGS("⚙", "SETTINGS"),
}

/**
 * home already shows the station at full size, so the strip would be a second
 * copy of the same fact. everywhere else it is the only way back to it.
 */
fun showsMiniPlayer(tab: Tab, stationName: String): Boolean =
    tab != Tab.HOME && stationName.isNotBlank()

/**
 * the four-tab shell every screen lives in. tab choice is rememberSaveable so
 * a rotation does not throw the user back to home, unlike plain remember.
 */
@Composable
fun R4dioApp(
    state: UiState,
    themeSlug: String,
    send: (String) -> Unit,
    onOpenSync: () -> Unit,
    keepAwake: Boolean = false,
    overlayOn: Boolean = false,
    onKeepAwake: (() -> Unit)? = null,
    onOverlay: (() -> Unit)? = null,
) {
    R4dioTheme(themeSlug) {
        val c = R4dioTokens.colors
        var tab by rememberSaveable { mutableStateOf(Tab.HOME) }
        Column(
            modifier = Modifier
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
                        onClearFilter = { send(CMD_CLEAR_FILTER) },
                        keepAwake = keepAwake,
                        overlayOn = overlayOn,
                        onKeepAwake = onKeepAwake,
                        onOverlay = onOverlay,
                    )
                    Tab.CATALOG -> Placeholder("CATALOG", "search and filters land here")
                    Tab.LIBRARY -> Placeholder("LIBRARY", "favourites and history land here")
                    Tab.SETTINGS -> SettingsPlaceholder(onOpenSync)
                }
            }
            if (showsMiniPlayer(tab, state.stationName)) {
                MiniPlayer(state)
            }
            TabBar(tab) { tab = it }
        }
    }
}

/** now-playing, reachable from every tab but home. tapping it does nothing yet. */
@Composable
private fun MiniPlayer(state: UiState) {
    val c = R4dioTokens.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(48.dp)
            .background(Color(c.panel()))
            .border(1.dp, Color(c.rule()))
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
                    text = tab.label,
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

/** settings placeholder, with the one live control this phase needs: the door to sync. */
@Composable
private fun SettingsPlaceholder(onOpenSync: () -> Unit) {
    val c = R4dioTokens.colors
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(c.bg))
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            text = "SETTINGS",
            color = Color(c.dim),
            fontSize = 16.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = MonoFamily,
            letterSpacing = 0.14.em,
        )
        Text(
            text = "preferences and themes land here",
            color = Color(c.dim),
            fontSize = 11.sp,
            fontFamily = MonoFamily,
            modifier = Modifier.padding(top = 8.dp, bottom = 20.dp),
        )
        Pill(text = "OPEN SYNC", on = true, onClick = onOpenSync)
    }
}
