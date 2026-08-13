package net.vchub.r4dio.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.vchub.r4dio.R
import net.vchub.r4dio.Station

/**
 * codec · bitrate. the separator rides on the field that follows, so a station
 * with no codec or an unknown bitrate never shows a dangling dot — the same
 * rule the home screen's context line carries.
 */
fun stationMeta(codec: String, bitrateLabel: String): String =
    listOf(codec.uppercase(), bitrateLabel)
        .filter { it.isNotBlank() }
        .joinToString(" · ")

/**
 * one row of the catalogue. tapping plays; long-pressing blocks.
 *
 * a long-press, not a swipe: a swipe inside a vertically scrolling list is easy
 * to trigger by accident reaching for the screen in a car, and blocking is
 * destructive.
 *
 * a blocked station stays listed, dimmed, with its own UNBLOCK affordance —
 * hiding it would leave the user no way to undo a block from the only screen
 * that lists stations.
 */
@Composable
fun StationRow(
    station: Station,
    starred: Boolean,
    blocked: Boolean,
    onPlay: () -> Unit,
    onStar: () -> Unit,
    onBlock: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = R4dioTokens.colors
    val shape = RoundedCornerShape(12.dp)
    val name = if (blocked) Color(c.dim) else Color(c.fg)
    val meta = if (blocked) Color(c.rule()) else Color(c.mute())
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(bottom = 6.dp)
            .background(Color(c.panel()), shape)
            .border(1.dp, Color(c.rule()), shape)
            // a blocked row does not play on a tap: the block is the user's
            // "never this one", and honouring it here is the whole point.
            .combinedClickable(
                onClick = { if (blocked) onBlock() else onPlay() },
                onLongClick = onBlock,
            )
            .padding(start = 12.dp, end = 6.dp, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = station.country.uppercase(),
            color = Color(if (blocked) c.rule() else c.accent),
            fontSize = 10.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = MonoFamily,
            letterSpacing = 0.1.em,
            textAlign = TextAlign.Center,
            maxLines = 1,
            modifier = Modifier.width(30.dp),
        )
        Column(modifier = Modifier.weight(1f).padding(start = 10.dp)) {
            Text(
                text = station.name,
                color = name,
                fontSize = 14.sp,
                fontFamily = MonoFamily,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.padding(top = 3.dp),
            ) {
                val rate = when {
                    station.bitrate > 0 -> stringResource(R.string.catalog_bitrate_k, station.bitrate)
                    else -> ""
                }
                val line = stationMeta(station.codec, rate)
                if (line.isNotBlank()) {
                    Text(
                        text = line,
                        color = meta,
                        fontSize = 10.sp,
                        fontFamily = MonoFamily,
                        letterSpacing = 0.06.em,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (blocked) {
                    Text(
                        text = stringResource(R.string.catalog_blocked),
                        color = Color(c.err),
                        fontSize = 9.sp,
                        fontWeight = FontWeight.Bold,
                        fontFamily = MonoFamily,
                        letterSpacing = 0.12.em,
                        maxLines = 1,
                    )
                }
            }
        }
        when (blocked) {
            true -> Pill(
                text = stringResource(R.string.catalog_unblock),
                on = true,
                onClick = onBlock,
                modifier = Modifier.padding(start = 6.dp, end = 6.dp),
            )
            false -> StarTarget(starred, onStar)
        }
    }
}

/** the star is its own 44dp target so a thumb aiming at it never plays the
 *  station instead. */
@Composable
private fun StarTarget(starred: Boolean, onStar: () -> Unit) {
    val c = R4dioTokens.colors
    Box(
        modifier = Modifier
            .defaultMinSize(minWidth = 44.dp, minHeight = 44.dp)
            .combinedClickable(onClick = onStar),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = if (starred) "★" else "☆",
            color = Color(if (starred) c.accent else c.dim),
            fontSize = 18.sp,
            fontFamily = MonoFamily,
        )
    }
}
