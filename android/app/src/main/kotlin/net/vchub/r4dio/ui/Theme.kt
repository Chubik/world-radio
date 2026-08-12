package net.vchub.r4dio.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ProvidableCompositionLocal
import androidx.compose.runtime.staticCompositionLocalOf

const val DEFAULT_THEME = "amber-crt"

/**
 * which theme to show, given what sync delivered and what we are showing now.
 * pure so the rule lives in one testable place: the synced value wins when this
 * build knows it, and is ignored otherwise rather than resetting the user.
 */
fun resolveTheme(synced: String, current: String): String {
    if (paletteFor(synced) != null) return synced
    if (paletteFor(current) != null) return current
    return DEFAULT_THEME
}

private val LocalPalette: ProvidableCompositionLocal<Palette> =
    staticCompositionLocalOf { paletteFor(DEFAULT_THEME)!! }

/**
 * the only place a colour enters the tree. screens read R4dioTokens.colors.x,
 * never a literal — a single hardcoded colour breaks all 14 themes at once.
 */
object R4dioTokens {
    val colors: Palette
        @Composable get() = LocalPalette.current
}

@Composable
fun R4dioTheme(slug: String, content: @Composable () -> Unit) {
    val palette = paletteFor(slug) ?: paletteFor(DEFAULT_THEME)!!
    CompositionLocalProvider(LocalPalette provides palette, content = content)
}
