package net.vchub.r4dio.ui

/**
 * the nine colour roles every r4dio client shares, mirroring
 * crates/radio-tui/src/tui/theme.rs. kept as 0xAARRGGBB longs rather than
 * compose Colors so this file stays a plain jvm unit and the values can be
 * compared against the rust source without a device.
 */
data class Palette(
    val bg: Long,
    val fg: Long,
    val accent: Long,
    val hot: Long,
    val dim: Long,
    val ok: Long,
    val err: Long,
    val info: Long,
    val peak: Long,
)

/** the 14 themes in the cli's cycle order. */
val THEME_SLUGS: List<String> = listOf(
    "amber-crt", "tube-glow", "hifi-paper", "shortwave-green", "cyber-neon",
    "atomic-terminal", "mainframe-blue", "nord", "gruvbox", "dracula",
    "solarized", "catppuccin", "rose-pine", "monokai",
)

private fun p(
    bg: Long, fg: Long, accent: Long, hot: Long, dim: Long,
    ok: Long, err: Long, info: Long, peak: Long,
) = Palette(
    bg or OPAQUE, fg or OPAQUE, accent or OPAQUE, hot or OPAQUE, dim or OPAQUE,
    ok or OPAQUE, err or OPAQUE, info or OPAQUE, peak or OPAQUE,
)

private const val OPAQUE = 0xFF000000

private val PALETTES: Map<String, Palette> = mapOf(
    "amber-crt" to p(0x15100B, 0xD49A3A, 0xFFC457, 0xFF8A3D, 0x6E5430, 0x9EC074, 0xD96A5A, 0x6FB0C8, 0xFFF0C0),
    "tube-glow" to p(0x0B1220, 0xE5D7B8, 0xFFE3A8, 0xFF8A4D, 0x6A6855, 0x7FD9A8, 0xFF6A6A, 0x5CC7D8, 0xFFF2CC),
    "hifi-paper" to p(0xEFE6CC, 0x2E2517, 0xC5872A, 0xA13E2D, 0x8A7A5A, 0x5A7A3A, 0xB14D2D, 0x2F6680, 0x0F0A04),
    "shortwave-green" to p(0x061008, 0x7FDA7F, 0xB5FF8A, 0xFF9D3D, 0x2D6633, 0x5FFF9C, 0xFF5C5C, 0x66C5FF, 0xD6FFC8),
    "cyber-neon" to p(0x07041A, 0xC7C0E8, 0x00FFE1, 0xFF2BD5, 0x463860, 0x6DFF7F, 0xFF5050, 0x5AD8FF, 0xFFFFFF),
    "atomic-terminal" to p(0x0A1A0C, 0x4CDC60, 0x9CFF66, 0xFFC232, 0x1F5E2A, 0x66FF5C, 0xFF5040, 0x5CFFAA, 0xD2FF8C),
    "mainframe-blue" to p(0x081A3A, 0xD8E8FF, 0x66C0FF, 0xFFD54A, 0x3A5A8A, 0x66E8A0, 0xFF7070, 0xFFB84D, 0xFFFFFF),
    "nord" to p(0x2E3440, 0xD8DEE9, 0x88C0D0, 0xD08770, 0x4C566A, 0xA3BE8C, 0xBF616A, 0x81A1C1, 0xECEFF4),
    "gruvbox" to p(0x282828, 0xEBDBB2, 0xFABD2F, 0xFE8019, 0x665C54, 0xB8BB26, 0xFB4934, 0x83A598, 0xFBF1C7),
    "dracula" to p(0x282A36, 0xF8F8F2, 0xBD93F9, 0xFF79C6, 0x6272A4, 0x50FA7B, 0xFF5555, 0x8BE9FD, 0xF1FA8C),
    "solarized" to p(0x002B36, 0x93A1A1, 0x268BD2, 0xCB4B16, 0x586E75, 0x859900, 0xDC322F, 0x2AA198, 0xFDF6E3),
    "catppuccin" to p(0x1E1E2E, 0xCDD6F4, 0xCBA6F7, 0xF5C2E7, 0x6C7086, 0xA6E3A1, 0xF38BA8, 0x89DCEB, 0xF9E2AF),
    "rose-pine" to p(0x191724, 0xE0DEF4, 0xC4A7E7, 0xEBBCBA, 0x6E6A86, 0x9CCFD8, 0xEB6F92, 0x31748F, 0xF6C177),
    "monokai" to p(0x272822, 0xF8F8F2, 0xA6E22E, 0xF92672, 0x75715E, 0xA6E22E, 0xF92672, 0x66D9EF, 0xE6DB74),
)

/**
 * null rather than a default: an unknown slug means a newer client picked a
 * theme this build does not have, and the caller must keep what it has rather
 * than silently resetting the user's choice.
 */
fun paletteFor(slug: String): Palette? = PALETTES[slug]

/**
 * the surface a panel sits on, one step off the background. derived rather
 * than a tenth slot so the 14 palettes stay a faithful copy of the cli's nine
 * roles. the factor reproduces today's @color/panel from amber-crt to within
 * one unit per channel (#1B1610 against #1B1510).
 */
fun Palette.panel(): Long = blend(bg, peak, 0.025f)

/**
 * the hairline between panels — the cli has no such role, so it is derived.
 * reproduces today's @color/rule (#392B1A against #3A2C17).
 */
fun Palette.rule(): Long = blend(bg, dim, 0.40f)

/**
 * secondary text: dimmer than fg, brighter than dim. blended toward peak
 * rather than fg because today's @color/mute is a near-neutral, not a
 * saturated amber; this reproduces it as #8A8066 against #8A7F64.
 */
fun Palette.mute(): Long = blend(bg, peak, 0.50f)

private fun blend(a: Long, b: Long, t: Float): Long {
    fun ch(shift: Int): Long {
        val av = (a shr shift) and 0xFF
        val bv = (b shr shift) and 0xFF
        return Math.round(av + ((bv - av) * t)).toLong() and 0xFF
    }
    return OPAQUE or (ch(16) shl 16) or (ch(8) shl 8) or ch(0)
}
