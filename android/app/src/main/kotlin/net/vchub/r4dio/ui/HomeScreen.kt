package net.vchub.r4dio.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.vchub.r4dio.R
import net.vchub.r4dio.catalogueLabel
import net.vchub.r4dio.filterIsInForce
import net.vchub.r4dio.filterPillLabel
import net.vchub.r4dio.isAllHiddenWarn
import net.vchub.r4dio.keepAwakeLabel
import net.vchub.r4dio.showsHiddenPill

/**
 * home, ported from activity_main.xml. every dimension here is that layout's;
 * it is the screen the user already knows and any drift from it is a defect.
 */
@Composable
fun HomeScreen(
    state: UiState,
    onShuffle: () -> Unit,
    onToggle: () -> Unit,
    onStar: () -> Unit,
    onScope: () -> Unit,
    onStop: () -> Unit,
    onSync: () -> Unit,
    onClearFilter: () -> Unit,
    keepAwake: Boolean = false,
    overlayOn: Boolean = false,
    onKeepAwake: (() -> Unit)? = null,
    onOverlay: (() -> Unit)? = null,
) {
    val c = R4dioTokens.colors
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(c.bg))
            // the four buttons and the sync bar are the eyes-free controls and
            // must never scroll away; the stage above them takes what is left and
            // scrolls internally, so a short screen loses nothing off the panel.
            .padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 16.dp),
    ) {
        Stage(
            state = state,
            onShuffle = onShuffle,
            onClearFilter = onClearFilter,
            keepAwake = keepAwake,
            overlayOn = overlayOn,
            onKeepAwake = onKeepAwake,
            onOverlay = onOverlay,
            modifier = Modifier.weight(1f),
        )
        ButtonRow(state, onToggle, onStar, onScope, onStop)
        SyncBar(onSync)
    }
}

@Composable
private fun Stage(
    state: UiState,
    onShuffle: () -> Unit,
    onClearFilter: () -> Unit,
    keepAwake: Boolean,
    overlayOn: Boolean,
    onKeepAwake: (() -> Unit)?,
    onOverlay: (() -> Unit)?,
    modifier: Modifier = Modifier,
) {
    val c = R4dioTokens.colors
    val shape = RoundedCornerShape(22.dp)
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(Color(c.panel()), shape)
            .border(1.dp, Color(c.rule()), shape)
            .clickable { onShuffle() }
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 22.dp, vertical = 20.dp),
    ) {
        Kicker(state.isPlaying)
        StationName(state.stationName)
        ContextLine(state)
        PillRow(state, onClearFilter, keepAwake, overlayOn, onKeepAwake, onOverlay)
        // the hero needs room to be the eyes-free target it exists to be. in
        // landscape the stage is only a couple of hundred dp tall, and a hero
        // with weight there pushed the pills and the station line out of the
        // panel entirely — the old landscape layout dropped it for the same
        // reason. the whole stage still shuffles on tap either way.
        Hero(state, modifier = Modifier.fillMaxWidth().height(HERO_HEIGHT))
    }
}

/** the ring plus its two lines. fixed rather than weighted so the stage can
 *  scroll: a weighted child inside a scrolling column has no height to take. */
private val HERO_HEIGHT = 300.dp

@Composable
private fun Kicker(isPlaying: Boolean) {
    val c = R4dioTokens.colors
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = if (isPlaying) "NOW PLAYING" else "PAUSED",
            color = Color(c.dim),
            fontSize = 10.sp,
            fontFamily = MonoFamily,
            letterSpacing = 0.22.em,
        )
        // the equaliser is the only "it is really playing" signal on the
        // screen that does not need reading, so it is hidden when paused
        // rather than greyed.
        if (isPlaying) {
            Equaliser()
        }
        Spacer(modifier = Modifier.weight(1f))
        Text(
            text = if (isPlaying) "LIVE" else "OFF AIR",
            color = Color(if (isPlaying) c.ok else c.mute()),
            fontSize = 10.sp,
            fontFamily = MonoFamily,
            letterSpacing = 0.22.em,
        )
    }
}

/** the four static bars from the kicker's @+id/eq, bottom aligned in a 14dp box. */
@Composable
private fun Equaliser() {
    val c = R4dioTokens.colors
    Row(
        modifier = Modifier.padding(start = 10.dp).height(14.dp),
        verticalAlignment = Alignment.Bottom,
        horizontalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        listOf(6.dp, 12.dp, 9.dp, 14.dp).forEach { h ->
            Box(
                modifier = Modifier
                    .width(2.5.dp)
                    .height(h)
                    .background(Color(c.fg)),
            )
        }
    }
}

@Composable
private fun StationName(name: String) {
    val c = R4dioTokens.colors
    Text(
        text = name.ifBlank { "— idle —" },
        color = Color(c.peak),
        fontSize = 30.sp,
        fontWeight = FontWeight.Bold,
        fontFamily = MonoFamily,
        letterSpacing = (-0.01).em,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        modifier = Modifier.fillMaxWidth().padding(top = 6.dp),
    )
}

/**
 * country · codec · favourite. the separator rides on the following field so an
 * absent country never leaves a dangling dot — the same rule MainActivity's
 * renderStation/renderFav pair carries today.
 */
@Composable
private fun ContextLine(state: UiState) {
    val c = R4dioTokens.colors
    val hasContext = state.country.isNotBlank() || state.codec.isNotBlank()
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (state.country.isNotBlank()) {
            Text(
                text = state.country,
                color = Color(c.mute()),
                fontSize = 11.sp,
                fontFamily = MonoFamily,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        if (state.codec.isNotBlank()) {
            Text(
                text = if (state.country.isNotBlank()) "· ${state.codec}" else state.codec,
                color = Color(c.mute()),
                fontSize = 11.sp,
                fontFamily = MonoFamily,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(start = if (state.country.isNotBlank()) 10.dp else 0.dp),
            )
        }
        Text(
            text = (if (hasContext) "· " else "") +
                if (state.isFav) "★ FAVOURITE" else "☆ not saved",
            color = Color(if (state.isFav) c.fg else c.mute()),
            fontSize = 11.sp,
            fontFamily = MonoFamily,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier
                .weight(1f)
                .padding(start = if (hasContext) 10.dp else 0.dp),
        )
    }
}

@Composable
private fun PillRow(
    state: UiState,
    onClearFilter: () -> Unit,
    keepAwake: Boolean,
    overlayOn: Boolean,
    onKeepAwake: (() -> Unit)?,
    onOverlay: (() -> Unit)?,
) {
    val c = R4dioTokens.colors
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val count = catalogueLabel(state.catalogueSize, state.catalogueGrowing)
        if (count.isNotEmpty()) {
            Text(
                text = count,
                color = Color(c.dim),
                fontSize = 9.5.sp,
                fontFamily = MonoFamily,
                letterSpacing = 0.1.em,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Spacer(modifier = Modifier.weight(1f))
        filterPillLabel(state.filterCountries, state.scope)?.let { label ->
            Pill(
                text = label,
                on = filterIsInForce(state.filterCountries, state.scope),
                onClick = onClearFilter,
                modifier = Modifier.padding(end = 6.dp),
            )
        }
        if (showsHiddenPill(state.hiddenCount, state.scope)) {
            Pill(
                text = "${state.hiddenCount} COUNTRIES",
                on = true,
                modifier = Modifier.padding(end = 6.dp),
            )
        }
        Pill(
            text = when {
                state.scope != "favs" -> "ALL STATIONS"
                state.favCount > 0 -> "FAVOURITES ONLY · ${state.favCount}"
                else -> "FAVOURITES ONLY"
            },
            on = state.scope == "favs",
        )
        Pill(
            text = keepAwakeLabel(keepAwake),
            on = keepAwake,
            onClick = onKeepAwake,
            description = stringResource(R.string.home_awake_desc),
            modifier = Modifier.padding(start = 6.dp),
        )
        Pill(
            text = if (overlayOn) "◧ SHOWS" else "◧ HIDDEN",
            on = overlayOn,
            onClick = onOverlay,
            description = stringResource(R.string.home_overlay_desc),
            modifier = Modifier.padding(start = 6.dp),
        )
    }
}

/**
 * the shuffle target. the ring turns danger-coloured when shuffle has nothing
 * to pick, which is the only warning the driver gets.
 */
@Composable
private fun Hero(state: UiState, modifier: Modifier = Modifier) {
    val c = R4dioTokens.colors
    val warn = warnMessage(state)
    val tone = Color(if (warn != null) c.err else c.accent)
    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            modifier = Modifier
                .size(168.dp)
                .background(tone.copy(alpha = RING_FILL_ALPHA), CircleShape)
                .border(2.dp, tone, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Image(
                painter = painterResource(R.drawable.ic_shuffle),
                contentDescription = null,
                colorFilter = ColorFilter.tint(tone),
                modifier = Modifier.size(108.dp),
            )
        }
        Text(
            text = "TAP ANYWHERE — SHUFFLE",
            color = tone,
            fontSize = 14.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = MonoFamily,
            letterSpacing = 0.14.em,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 18.dp),
        )
        Text(
            text = warn ?: if (state.scope == "favs") {
                "random favourite · eyes-free"
            } else {
                "random station · eyes-free"
            },
            color = Color(if (warn != null) c.err else c.dim),
            fontSize = 10.5.sp,
            fontFamily = MonoFamily,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 6.dp),
        )
    }
}

/** the two reasons shuffle has nothing to pick, each with its own message. */
private fun warnMessage(state: UiState): String? = when {
    state.scope == "favs" && state.favCount == 0 -> "NO FAVOURITES YET — STAR ONE FIRST"
    isAllHiddenWarn(
        state.playableCount,
        state.hiddenCount,
        state.scope,
        state.catalogLoaded,
    ) -> "NO STATIONS — ALL COUNTRIES HIDDEN"
    else -> null
}

@Composable
private fun ButtonRow(
    state: UiState,
    onToggle: () -> Unit,
    onStar: () -> Unit,
    onScope: () -> Unit,
    onStop: () -> Unit,
) {
    val c = R4dioTokens.colors
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 14.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        SecButton(
            icon = if (state.isPlaying) R.drawable.ic_pause else R.drawable.ic_play,
            label = if (state.isPlaying) "PAUSE" else "PLAY",
            tone = Color(c.peak),
            on = false,
            onClick = onToggle,
            modifier = Modifier.weight(1f),
        )
        SecButton(
            icon = if (state.isFav) R.drawable.ic_star else R.drawable.ic_star_outline,
            label = if (state.isFav) "STARRED" else "STAR",
            tone = Color(c.peak),
            on = state.isFav,
            onClick = onStar,
            modifier = Modifier.weight(1f),
        )
        SecButton(
            icon = if (state.scope == "favs") {
                R.drawable.ic_scope_favs
            } else {
                R.drawable.ic_scope_all
            },
            label = "scope",
            tone = Color(c.peak),
            on = false,
            onClick = onScope,
            modifier = Modifier.weight(1f),
        )
        SecButton(
            icon = R.drawable.ic_stop,
            label = "STOP",
            tone = Color(c.err),
            on = false,
            onClick = onStop,
            modifier = Modifier.weight(1f),
        )
    }
}

/**
 * one of the four bottom controls. 66dp minimum height is a driving target,
 * not a style choice — it is what makes the button findable without looking.
 */
@Composable
private fun SecButton(
    icon: Int,
    label: String,
    tone: Color,
    on: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = R4dioTokens.colors
    val shape = RoundedCornerShape(14.dp)
    val fill = if (on) Color(c.accent).copy(alpha = BTN_ON_ALPHA) else Color(c.panel())
    val stroke = if (on) Color(c.fg) else Color(c.rule())
    Column(
        modifier = modifier
            .defaultMinSize(minHeight = 66.dp)
            .background(fill, shape)
            .border(1.dp, stroke, shape)
            .clickable { onClick() }
            .padding(start = 6.dp, end = 6.dp, top = 12.dp, bottom = 9.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Image(
            painter = painterResource(icon),
            contentDescription = null,
            colorFilter = ColorFilter.tint(tone),
            modifier = Modifier.size(26.dp),
        )
        Text(
            text = label,
            color = tone,
            fontSize = 10.sp,
            fontWeight = FontWeight.SemiBold,
            fontFamily = MonoFamily,
            maxLines = 1,
            modifier = Modifier.padding(top = 6.dp),
        )
    }
}

@Composable
private fun SyncBar(onSync: () -> Unit) {
    val c = R4dioTokens.colors
    val shape = RoundedCornerShape(14.dp)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 10.dp)
            .height(56.dp)
            .border(1.dp, Color(c.fg), shape)
            .clickable { onSync() }
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Image(
            painter = painterResource(R.drawable.ic_sync),
            contentDescription = null,
            colorFilter = ColorFilter.tint(Color(c.fg)),
            modifier = Modifier.size(22.dp),
        )
        Text(
            text = "SYNC",
            color = Color(c.fg),
            fontSize = 15.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = MonoFamily,
            letterSpacing = 0.16.em,
            modifier = Modifier.padding(start = 10.dp),
        )
        Spacer(modifier = Modifier.weight(1f))
        Text(
            text = "link desktop ↔ phone",
            color = Color(c.dim),
            fontSize = 10.5.sp,
            fontFamily = MonoFamily,
            textAlign = TextAlign.End,
        )
    }
}

/** bg_hero_ring's #1E fill prefix, as a fraction. */
private const val RING_FILL_ALPHA = 0x1E / 255f

/** bg_sec_btn_on's #17 fill prefix, as a fraction. */
private const val BTN_ON_ALPHA = 0x17 / 255f
