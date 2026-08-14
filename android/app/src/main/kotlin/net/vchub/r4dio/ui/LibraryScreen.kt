package net.vchub.r4dio.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.vchub.r4dio.R
import net.vchub.r4dio.Station

/** the three lists the library holds. */
enum class LibraryTab { FAVOURITES, BLOCKED, HISTORY }

/**
 * favourites, blocked and history under one segmented control.
 *
 * every list reuses [StationRow], so a station reads the same here as in the
 * catalogue — including the star and the block, which stay actionable rather
 * than becoming read-only rows.
 */
@Composable
fun LibraryScreen(
    favourites: List<Station>,
    blocked: List<Station>,
    history: List<Station>,
    favouriteUuids: Set<String>,
    blockedUuids: Set<String>,
    onPlay: (Station) -> Unit,
    onStar: (Station) -> Unit,
    onBlock: (Station) -> Unit,
    onClearHistory: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = R4dioTokens.colors
    var tab by rememberSaveable { mutableStateOf(LibraryTab.FAVOURITES) }
    val shown = when (tab) {
        LibraryTab.FAVOURITES -> favourites
        LibraryTab.BLOCKED -> blocked
        LibraryTab.HISTORY -> history
    }
    // each list is its own scroll position: switching tabs and coming back to
    // where you were is the whole point of a segmented control.
    val listState = rememberLazyListState(0, 0)

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Color(c.bg))
            .padding(horizontal = 16.dp),
    ) {
        TabRow(
            tab = tab,
            counts = Triple(favourites.size, blocked.size, history.size),
            onPick = { tab = it },
            showClear = tab == LibraryTab.HISTORY && history.isNotEmpty(),
            onClear = onClearHistory,
        )
        when {
            shown.isEmpty() -> EmptyLibrary(tab)
            else -> LazyColumn(state = listState, modifier = Modifier.fillMaxSize()) {
                items(shown, key = { it.uuid }) { station ->
                    StationRow(
                        station = station,
                        starred = station.uuid in favouriteUuids,
                        blocked = station.uuid in blockedUuids,
                        onPlay = { onPlay(station) },
                        onStar = { onStar(station) },
                        onBlock = { onBlock(station) },
                    )
                }
            }
        }
    }
}

/**
 * the counts are on the pills themselves: "BLOCKED 3" answers "have i blocked
 * anything" without a tap, which is the question the tab exists for.
 */
@Composable
private fun TabRow(
    tab: LibraryTab,
    counts: Triple<Int, Int, Int>,
    onPick: (LibraryTab) -> Unit,
    showClear: Boolean,
    onClear: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 14.dp, bottom = 10.dp)
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        LibraryTab.entries.forEach { entry ->
            val count = when (entry) {
                LibraryTab.FAVOURITES -> counts.first
                LibraryTab.BLOCKED -> counts.second
                LibraryTab.HISTORY -> counts.third
            }
            val label = stringResource(
                when (entry) {
                    LibraryTab.FAVOURITES -> R.string.library_favourites
                    LibraryTab.BLOCKED -> R.string.library_blocked
                    LibraryTab.HISTORY -> R.string.library_history
                },
            )
            Pill(
                text = if (count > 0) "$label $count" else label,
                on = entry == tab,
                onClick = { onPick(entry) },
            )
        }
        if (showClear) {
            Spacer(modifier = Modifier.weight(1f))
            Pill(text = stringResource(R.string.library_clear), on = false, onClick = onClear)
        }
    }
}

/**
 * an empty list needs to say which emptiness it is: nothing starred yet reads
 * very differently from a history that has not started.
 */
@Composable
private fun EmptyLibrary(tab: LibraryTab) {
    val c = R4dioTokens.colors
    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            text = stringResource(
                when (tab) {
                    LibraryTab.FAVOURITES -> R.string.library_empty_favourites
                    LibraryTab.BLOCKED -> R.string.library_empty_blocked
                    LibraryTab.HISTORY -> R.string.library_empty_history
                },
            ),
            color = Color(c.dim),
            fontSize = 13.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = MonoFamily,
            letterSpacing = 0.12.em,
        )
        Text(
            text = stringResource(
                when (tab) {
                    LibraryTab.FAVOURITES -> R.string.library_hint_favourites
                    LibraryTab.BLOCKED -> R.string.library_hint_blocked
                    LibraryTab.HISTORY -> R.string.library_hint_history
                },
            ),
            color = Color(c.mute()),
            fontSize = 11.sp,
            fontFamily = MonoFamily,
            modifier = Modifier.padding(top = 8.dp),
        )
    }
}
