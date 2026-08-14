package net.vchub.r4dio.ui

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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

/**
 * the playing station, full screen.
 *
 * every action here already exists as a session command, so this screen adds no
 * new command — which is the one place this codebase reliably goes wrong, since
 * a command must be registered in two separate lists to work at all.
 */
@Composable
fun NowPlayingScreen(
    state: UiState,
    onToggle: () -> Unit,
    onShuffle: () -> Unit,
    onStar: () -> Unit,
    onBlock: () -> Unit,
    onStop: () -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = R4dioTokens.colors
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Color(c.bg))
            // this screen is drawn over the shell, outside the inset padding the
            // tab content sits inside — without its own, the close pill lands
            // under the status bar clock.
            .windowInsetsPadding(WindowInsets.systemBars)
            .padding(horizontal = 20.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(top = 14.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Pill(text = stringResource(R.string.now_close), on = false, onClick = onClose)
            Spacer(modifier = Modifier.weight(1f))
            Text(
                text = stringResource(
                    if (state.isPlaying) R.string.now_live else R.string.now_paused,
                ),
                color = Color(if (state.isPlaying) c.ok else c.dim),
                fontSize = 11.sp,
                fontFamily = MonoFamily,
                letterSpacing = 0.16.em,
            )
        }

        Column(
            modifier = Modifier.weight(1f).fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Bars(playing = state.isPlaying)
            Text(
                text = state.stationName.ifBlank { stringResource(R.string.now_nothing) },
                color = Color(c.fg),
                fontSize = 26.sp,
                fontWeight = FontWeight.Bold,
                fontFamily = MonoFamily,
                textAlign = TextAlign.Center,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 28.dp),
            )
            val meta = nowPlayingMeta(state.country, state.codec)
            if (meta.isNotEmpty()) {
                Text(
                    text = meta,
                    color = Color(c.mute()),
                    fontSize = 12.sp,
                    fontFamily = MonoFamily,
                    letterSpacing = 0.1.em,
                    modifier = Modifier.padding(top = 10.dp),
                )
            }
            if (state.stationUuid.isNotBlank()) {
                Row(
                    modifier = Modifier.padding(top = 22.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Pill(
                        text = stringResource(
                            if (state.isFav) R.string.now_starred else R.string.now_star,
                        ),
                        on = state.isFav,
                        onClick = onStar,
                    )
                    Pill(text = stringResource(R.string.now_block), on = false, onClick = onBlock)
                }
            }
        }

        Row(
            modifier = Modifier.fillMaxWidth().padding(bottom = 22.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Control(stringResource(R.string.now_shuffle), onShuffle, Modifier.weight(1f))
            Control(
                stringResource(if (state.isPlaying) R.string.now_pause else R.string.now_play),
                onToggle,
                Modifier.weight(1f),
            )
            Control(stringResource(R.string.now_stop), onStop, Modifier.weight(1f), hot = true)
        }
    }
}

/** the meta line, skipping whatever the station did not report. */
fun nowPlayingMeta(country: String, codec: String): String =
    listOf(country, codec).filter { it.isNotBlank() }.joinToString(" · ")

/**
 * bars that move while audio is playing and settle when it is not.
 *
 * deliberately driven by playback state rather than the audio itself: a real
 * analyser needs RECORD_AUDIO, and a radio app asking for microphone access
 * reads as something else entirely. this says the one true thing a listener
 * wants confirmed — sound is coming out.
 */
@Composable
private fun Bars(playing: Boolean) {
    val c = R4dioTokens.colors
    val transition = rememberInfiniteTransition(label = "bars")
    // each bar gets its own period so the row never pulses as one block.
    val periods = listOf(620, 430, 780, 520, 340)
    val heights = periods.mapIndexed { index, period ->
        transition.animateFloat(
            initialValue = 0.25f,
            targetValue = 1f,
            animationSpec = infiniteRepeatable(
                animation = tween(durationMillis = period),
                repeatMode = RepeatMode.Reverse,
            ),
            label = "bar$index",
        )
    }
    Row(
        modifier = Modifier.height(88.dp),
        horizontalArrangement = Arrangement.spacedBy(7.dp),
        verticalAlignment = Alignment.Bottom,
    ) {
        heights.forEach { height ->
            // a settled bar is a short stub rather than nothing: an empty row
            // would read as "no station" instead of "paused".
            val factor = if (playing) height.value else 0.18f
            Box(
                modifier = Modifier
                    .size(width = 11.dp, height = (88.dp * factor))
                    .background(
                        Color(if (playing) c.accent else c.dim),
                        RoundedCornerShape(2.dp),
                    ),
            )
        }
    }
}

@Composable
private fun Control(label: String, onClick: () -> Unit, modifier: Modifier, hot: Boolean = false) {
    val c = R4dioTokens.colors
    val shape = RoundedCornerShape(14.dp)
    Box(
        modifier = modifier
            .height(56.dp)
            .background(Color(c.panel()), shape)
            .border(1.dp, Color(c.rule()), shape)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color = Color(if (hot) c.hot else c.accent),
            fontSize = 12.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = MonoFamily,
            letterSpacing = 0.12.em,
        )
    }
}
