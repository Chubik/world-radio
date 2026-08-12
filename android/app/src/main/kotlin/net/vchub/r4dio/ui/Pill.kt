package net.vchub.r4dio.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.vchub.r4dio.R

val MonoFamily = FontFamily(Font(R.font.ibm_plex_mono))

/**
 * the status pill from the current release, unchanged in proportion: 9.5sp
 * mono, 0.1 letter spacing, 9dp/3dp padding. [on] is the amber state, off is
 * dim — the difference is what tells the user a filter is or is not in force.
 *
 * the tracking is .em, not .sp: android:letterSpacing in the layout this was
 * ported from is a multiple of the font size, so 0.1.sp would set it ten times
 * tighter than the release.
 *
 * the two states are the bg_pill / bg_pill_on drawables: off is the panel fill
 * with a rule hairline, on is accent at 9% with an accent hairline. the alpha
 * is the drawables' #17 prefix, kept so a pill reads the same over any theme's
 * background.
 */
@Composable
fun Pill(
    text: String,
    on: Boolean,
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
) {
    val c = R4dioTokens.colors
    val fg = if (on) Color(c.accent) else Color(c.dim)
    val stroke = if (on) Color(c.fg) else Color(c.rule())
    val fill = if (on) Color(c.accent).copy(alpha = PILL_ON_ALPHA) else Color(c.panel())
    val shape = RoundedCornerShape(PILL_RADIUS)
    Text(
        text = text,
        color = fg,
        fontSize = 9.5.sp,
        fontFamily = MonoFamily,
        letterSpacing = 0.1.em,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        modifier = modifier
            .background(fill, shape)
            .border(1.dp, stroke, shape)
            .let { if (onClick != null) it.clickable { onClick() } else it }
            .padding(horizontal = 9.dp, vertical = 3.dp),
    )
}

/** bg_pill_on's #17 fill prefix, as a fraction. */
private const val PILL_ON_ALPHA = 0x17 / 255f

private val PILL_RADIUS = 20.dp
